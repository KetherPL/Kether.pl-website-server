// SPDX-License-Identifier: GPL-3.0-only

use crate::config::{CONF_FILE_NAME, Config, ConfigChange};
use crate::steam_bot::registry::ConfigHandle;
use notify_debouncer_full::notify::{
	EventKind, RecommendedWatcher, RecursiveMode,
	event::{AccessKind, AccessMode, ModifyKind},
};
use notify_debouncer_full::{
	DebounceEventResult, DebouncedEvent, Debouncer, RecommendedCache, new_debouncer,
};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// Last successfully observed on-disk identity for config.toml.
/// Used to skip redundant reloads when metadata is unchanged.
static LAST_FILE_IDENTITY: Mutex<Option<(SystemTime, u64)>> = Mutex::new(None);

fn file_identity(path: &Path) -> Result<(SystemTime, u64), String> {
	let metadata = std::fs::metadata(path)
		.map_err(|e| format!("Failed to stat {}: {}", path.display(), e))?;
	let modified = metadata
		.modified()
		.map_err(|e| format!("Failed to read mtime for {}: {}", path.display(), e))?;
	Ok((modified, metadata.len()))
}

fn identity_unchanged(path: &Path) -> Result<bool, String> {
	let identity = file_identity(path)?;
	Ok(LAST_FILE_IDENTITY
		.lock()
		.map_err(|e| format!("Config watcher state lock poisoned: {}", e))?
		.as_ref() == Some(&identity))
}

fn remember_identity(path: &Path) -> Result<(), String> {
	*LAST_FILE_IDENTITY
		.lock()
		.map_err(|e| format!("Config watcher state lock poisoned: {}", e))? =
		Some(file_identity(path)?);
	Ok(())
}

pub fn apply_reload(handle: &ConfigHandle, path: &Path) -> Result<ConfigChange, String> {
	if identity_unchanged(path)? {
		return Ok(ConfigChange {
			unchanged: true,
			..ConfigChange::default()
		});
	}

	let mut new_config = {
		let content = std::fs::read_to_string(path)
			.map_err(|e| format!("Failed to reload config: {}", e))?;
		Config::from_toml_str(&content).map_err(|e| format!("Failed to reload config: {}", e))?
	};

	let change = match handle.write() {
		Ok(mut guard) => {
			let previous = guard.clone();
			new_config.prepare_for_runtime(Some(&previous));
			let change = previous.diff(&new_config);
			if !change.unchanged {
				*guard = Arc::new(new_config);
			}
			change
		}
		Err(e) => {
			let mut guard = e.into_inner();
			let previous = guard.clone();
			new_config.prepare_for_runtime(Some(&previous));
			let change = previous.diff(&new_config);
			if !change.unchanged {
				*guard = Arc::new(new_config);
			}
			change
		}
	};

	remember_identity(path)?;
	Ok(change)
}

fn event_mentions_config(event: &DebouncedEvent, config_path: &Path) -> bool {
	let expected_name = config_path
		.file_name()
		.and_then(|n| n.to_str())
		.unwrap_or(CONF_FILE_NAME);
	event.event.paths.iter().any(|path| {
		path == config_path || path.file_name().and_then(|n| n.to_str()) == Some(expected_name)
	})
}

/// Returns true for filesystem events that indicate config content may have changed.
///
/// Ignores `Access(Open)` and `Access(Close(Read))`, which are emitted when this process
/// reads config.toml during reload and would otherwise cause a reload feedback loop.
fn is_substantive_config_event(event: &DebouncedEvent, config_path: &Path) -> bool {
	if !event_mentions_config(event, config_path) {
		return false;
	}

	match event.kind {
		EventKind::Modify(ModifyKind::Data(_)) => true,
		EventKind::Modify(ModifyKind::Name(_)) => true,
		EventKind::Create(_) => true,
		EventKind::Remove(_) => true,
		EventKind::Access(AccessKind::Close(AccessMode::Write)) => true,
		EventKind::Modify(ModifyKind::Metadata(
			notify_debouncer_full::notify::event::MetadataKind::WriteTime,
		)) => true,
		_ => false,
	}
}

pub fn spawn_config_watcher(
	handle: ConfigHandle,
	config_path: PathBuf,
) -> Result<Debouncer<RecommendedWatcher, RecommendedCache>, String> {
	let watch_parent = config_path
		.parent()
		.ok_or_else(|| "Config path has no parent directory".to_string())?
		.to_path_buf();
	let watched_path = config_path.clone();

	// Seed identity so the first debounced noise does not reload immediately.
	if let Err(err) = remember_identity(&watched_path) {
		eprintln!("Config watcher: failed to seed file identity: {}", err);
	}

	let mut debouncer = new_debouncer(
		Duration::from_secs(1),
		None,
		move |result: DebounceEventResult| match result {
			Ok(events) => {
				let relevant = events
					.iter()
					.any(|event| is_substantive_config_event(event, &watched_path));
				if !relevant {
					return;
				}

				match apply_reload(&handle, &watched_path) {
					Ok(change) => change.log(),
					Err(err) => eprintln!("Config hot reload failed: {}", err),
				}
			}
			Err(errors) => {
				for err in errors {
					eprintln!("Config watcher error: {}", err);
				}
			}
		},
	)
	.map_err(|e| format!("Failed to create config watcher: {}", e))?;

	debouncer
		.watch(&watch_parent, RecursiveMode::NonRecursive)
		.map_err(|e| format!("Failed to watch config directory {}: {}", watch_parent.display(), e))?;

	Ok(debouncer)
}

#[cfg(test)]
mod tests {
	use super::*;
	use notify_debouncer_full::notify::Event;
	use std::sync::RwLock;

	fn write_config(path: &Path, content: &str) {
		std::fs::write(path, content).expect("failed to write config fixture");
	}

	fn base_toml(ip: &str, password: &str) -> String {
		format!(
			r#"
frontend_admins = [76561198000000001]

[server]
ip = "{ip}"
port = 27015

[server2]
ip = "127.0.0.2"
port = 27016

[steam]
web_api_key = "key1"

[steam.bot]
username = "botuser"
password = "{password}"

[steam.chat]
group_id = 123
chat_id = 456
commands_without_mention = false
"#
		)
	}

	fn reset_identity_cache() {
		*LAST_FILE_IDENTITY.lock().expect("identity lock") = None;
	}

	#[test]
	fn apply_reload_updates_snapshot_and_returns_diff() {
		reset_identity_cache();
		let tmp = tempfile::tempdir().expect("tempdir");
		let config_path = tmp.path().join(CONF_FILE_NAME);
		write_config(&config_path, &base_toml("127.0.0.1", "pw1"));

		let initial = Config::load_from(&config_path).expect("initial load");
		let handle: ConfigHandle = Arc::new(RwLock::new(Arc::new(initial)));

		write_config(&config_path, &base_toml("127.0.0.9", "pw1"));
		let change = apply_reload(&handle, &config_path).expect("apply reload");
		assert!(!change.unchanged);
		assert!(change.live_applied.contains(&"server.ip"));
		assert!(change.requires_restart.is_empty());

		let snapshot = handle.read().expect("read lock").clone();
		assert_eq!(snapshot.server_ip, "127.0.0.9");
	}

	#[test]
	fn apply_reload_keeps_old_snapshot_on_invalid_toml() {
		reset_identity_cache();
		let tmp = tempfile::tempdir().expect("tempdir");
		let config_path = tmp.path().join(CONF_FILE_NAME);
		write_config(&config_path, &base_toml("127.0.0.1", "pw1"));

		let initial = Config::load_from(&config_path).expect("initial load");
		let handle: ConfigHandle = Arc::new(RwLock::new(Arc::new(initial)));
		remember_identity(&config_path).expect("seed identity");

		write_config(&config_path, "not [valid");
		let result = apply_reload(&handle, &config_path);
		assert!(result.is_err());

		let snapshot = handle.read().expect("read lock").clone();
		assert_eq!(snapshot.server_ip, "127.0.0.1");
	}

	#[test]
	fn apply_reload_preserves_old_arc_snapshots() {
		reset_identity_cache();
		let tmp = tempfile::tempdir().expect("tempdir");
		let config_path = tmp.path().join(CONF_FILE_NAME);
		write_config(&config_path, &base_toml("127.0.0.1", "pw1"));

		let initial = Config::load_from(&config_path).expect("initial load");
		let handle: ConfigHandle = Arc::new(RwLock::new(Arc::new(initial)));
		let old_snapshot = handle.read().expect("read lock").clone();

		write_config(&config_path, &base_toml("127.0.0.2", "pw2"));
		let change = apply_reload(&handle, &config_path).expect("apply reload");
		assert!(change.live_applied.contains(&"server.ip"));
		assert!(change.requires_restart.contains(&"steam.bot.password"));

		assert_eq!(old_snapshot.server_ip, "127.0.0.1");
		assert_eq!(old_snapshot.steam_password, "pw1");

		let new_snapshot = handle.read().expect("read lock").clone();
		assert_eq!(new_snapshot.server_ip, "127.0.0.2");
		assert_eq!(new_snapshot.steam_password, "pw2");
	}

	#[test]
	fn apply_reload_skips_when_file_identity_unchanged() {
		reset_identity_cache();
		let tmp = tempfile::tempdir().expect("tempdir");
		let config_path = tmp.path().join(CONF_FILE_NAME);
		write_config(&config_path, &base_toml("127.0.0.1", "pw1"));

		let initial = Config::load_from(&config_path).expect("initial load");
		let handle: ConfigHandle = Arc::new(RwLock::new(Arc::new(initial)));
		remember_identity(&config_path).expect("seed identity");

		let change = apply_reload(&handle, &config_path).expect("second apply");
		assert!(change.unchanged);
	}

	#[test]
	fn substantive_event_filter_ignores_open_and_close_read() {
		let config_path = PathBuf::from("/tmp/config.toml");
		let open = DebouncedEvent::new(
			Event::new(EventKind::Access(AccessKind::Open(AccessMode::Read)))
				.add_path(config_path.clone()),
			std::time::Instant::now(),
		);
		let close_read = DebouncedEvent::new(
			Event::new(EventKind::Access(AccessKind::Close(AccessMode::Read)))
				.add_path(config_path.clone()),
			std::time::Instant::now(),
		);
		let modify = DebouncedEvent::new(
			Event::new(EventKind::Modify(ModifyKind::Data(
				notify_debouncer_full::notify::event::DataChange::Any,
			)))
			.add_path(config_path),
			std::time::Instant::now(),
		);

		assert!(!is_substantive_config_event(&open, &PathBuf::from("/tmp/config.toml")));
		assert!(!is_substantive_config_event(
			&close_read,
			&PathBuf::from("/tmp/config.toml")
		));
		assert!(is_substantive_config_event(
			&modify,
			&PathBuf::from("/tmp/config.toml")
		));
	}

	#[test]
	fn watcher_applies_changes_end_to_end() {
		reset_identity_cache();
		let tmp = tempfile::tempdir().expect("tempdir");
		let config_path = tmp.path().join(CONF_FILE_NAME);
		write_config(&config_path, &base_toml("127.0.0.1", "pw1"));

		let initial = Config::load_from(&config_path).expect("initial load");
		let handle: ConfigHandle = Arc::new(RwLock::new(Arc::new(initial)));

		let _watcher =
			spawn_config_watcher(handle.clone(), config_path.clone()).expect("watcher start");
		std::thread::sleep(Duration::from_millis(200));
		write_config(&config_path, &base_toml("127.0.0.42", "pw1"));

		let deadline = std::time::Instant::now() + Duration::from_secs(3);
		loop {
			let snapshot = handle.read().expect("read lock").clone();
			if snapshot.server_ip == "127.0.0.42" {
				break;
			}
			assert!(
				std::time::Instant::now() < deadline,
				"watcher did not apply update in time"
			);
			std::thread::sleep(Duration::from_millis(50));
		}
	}
}
