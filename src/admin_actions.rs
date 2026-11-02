// SPDX-License-Identifier: GPL-3.0-only

//! Bounded, persistent history of Admin Panel maintenance actions.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use chrono::Utc;
use rocket::serde::json::{self, Value};
use rocket::serde::{Deserialize, Serialize};
use tokio::sync::Mutex;

pub const LOG_FILE_NAME: &str = "log_admin_actions.json";
static TEMP_SEQUENCE: AtomicU64 = AtomicU64::new(0);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(crate = "rocket::serde", rename_all = "snake_case")]
pub enum ActionChoice {
	Start,
	Stop,
	Restart,
	SoftRestart,
}

#[derive(Debug, Clone, Copy)]
#[repr(u16)]
pub enum AdminAction {
	Backend = 1,
	Daemon = 2,
	GameServer = 3,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct ActionDefinition {
	pub id: u16,
	pub key: &'static str,
	pub choices: &'static [ActionChoice],
	pub modifiers: &'static [&'static str],
}

/// IDs are persistent contracts: add new IDs rather than reassigning existing ones.
pub const ACTION_DEFINITIONS: &[ActionDefinition] = &[
	ActionDefinition {
		id: AdminAction::Backend as u16,
		key: "backend",
		choices: &[ActionChoice::Restart, ActionChoice::SoftRestart],
		modifiers: &[],
	},
	ActionDefinition {
		id: AdminAction::Daemon as u16,
		key: "daemon",
		choices: &[ActionChoice::Restart, ActionChoice::SoftRestart],
		modifiers: &["force"],
	},
	ActionDefinition {
		id: AdminAction::GameServer as u16,
		key: "game_server",
		choices: &[ActionChoice::Start, ActionChoice::Stop, ActionChoice::Restart],
		modifiers: &[],
	},
];

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(crate = "rocket::serde", rename_all = "snake_case")]
pub enum ActionOutcome {
	Requested,
	Accepted,
	Completed,
	Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminActionEntry {
	pub id: u64,
	/// String representation preserves the full SteamID64 in JavaScript clients.
	pub steam_id: String,
	pub action_id: u16,
	pub choice: ActionChoice,
	pub modifiers: BTreeMap<String, bool>,
	/// Reserved for action-specific results, such as a service or future map/bind data.
	pub additional_data: Option<Value>,
	pub unix_time: i64,
	pub outcome: ActionOutcome,
	pub error: Option<String>,
}

pub struct AdminActionLog {
	path: PathBuf,
	lock: Mutex<()>,
}

impl AdminActionLog {
	pub fn new(path: PathBuf) -> Self {
		Self { path, lock: Mutex::new(()) }
	}

	fn read(&self) -> Result<Vec<AdminActionEntry>, String> {
		match std::fs::read(&self.path) {
			Ok(content) => json::from_slice(&content)
				.map_err(|error| format!("Invalid admin action log: {error}")),
			Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
				self.write(&[])?;
				Ok(Vec::new())
			}
			Err(error) => Err(format!("Cannot read admin action log: {error}")),
		}
	}

	fn write(&self, entries: &[AdminActionEntry]) -> Result<(), String> {
		let content = json::to_pretty_string(&entries).map_err(|error| error.to_string())?;
		let sequence = TEMP_SEQUENCE.fetch_add(1, Ordering::Relaxed);
		let temporary = self.path.with_extension(format!("{}.{}.tmp", std::process::id(), sequence));
		let mut options = std::fs::OpenOptions::new();
		options.write(true).create_new(true);
		#[cfg(unix)]
		{
			use std::os::unix::fs::OpenOptionsExt;
			options.mode(0o600);
		}
		let mut file = options.open(&temporary)
			.map_err(|error| format!("Cannot create admin action log temporary file: {error}"))?;
		let result = (|| {
			file.write_all(content.as_bytes())?;
			file.sync_all()?;
			std::fs::rename(&temporary, &self.path)
		})();
		if let Err(error) = result {
			let _ = std::fs::remove_file(&temporary);
			return Err(format!("Cannot save admin action log: {error}"));
		}
		Ok(())
	}

	pub async fn begin(
		&self,
		config: &crate::config::Config,
		steam_id: i64,
		action: AdminAction,
		choice: ActionChoice,
		force: bool,
	) -> Result<Option<u64>, String> {
		if !config.admin_action_logging {
			return Ok(None);
		}
		let definition = ACTION_DEFINITIONS.iter().find(|definition| definition.id == action as u16)
			.ok_or_else(|| "Unknown admin action".to_string())?;
		if !definition.choices.contains(&choice) || (force && !definition.modifiers.contains(&"force")) {
			return Err("Unsupported admin action choice or modifier".to_string());
		}
		let _guard = self.lock.lock().await;
		let mut entries = self.read()?;
		let id = entries.iter().map(|entry| entry.id).max().unwrap_or(0)
			.checked_add(1).ok_or_else(|| "Admin action log ID exhausted".to_string())?;
		let mut modifiers = BTreeMap::new();
		if definition.modifiers.contains(&"force") {
			modifiers.insert("force".to_string(), force);
		}
		entries.push(AdminActionEntry {
			id,
			steam_id: steam_id.to_string(),
			action_id: action as u16,
			choice,
			modifiers,
			additional_data: None,
			unix_time: Utc::now().timestamp(),
			outcome: ActionOutcome::Requested,
			error: None,
		});
		let excess = entries.len().saturating_sub(config.admin_action_logging_max);
		entries.drain(..excess);
		self.write(&entries)?;
		Ok(Some(id))
	}

	pub async fn finish(
		&self,
		id: Option<u64>,
		outcome: ActionOutcome,
		additional_data: Option<Value>,
		error: Option<String>,
	) -> Result<(), String> {
		let Some(id) = id else { return Ok(()) };
		let _guard = self.lock.lock().await;
		let mut entries = self.read()?;
		// A concurrent burst may already have evicted this entry. Never reinsert old entries.
		if let Some(entry) = entries.iter_mut().find(|entry| entry.id == id) {
			entry.outcome = outcome;
			entry.additional_data = additional_data;
			entry.error = error;
			self.write(&entries)?;
		}
		Ok(())
	}

	pub async fn history(&self, maximum: usize) -> Result<Vec<AdminActionEntry>, String> {
		let _guard = self.lock.lock().await;
		Ok(self.read()?.into_iter().rev().take(maximum).collect())
	}
}

pub fn default_log_path() -> Result<PathBuf, String> {
	crate::config::exe_dir().map(|directory| directory.join(Path::new(LOG_FILE_NAME)))
		.map_err(|error| format!("Cannot determine admin action log path: {error}"))
}

#[cfg(test)]
mod tests {
	use super::*;

	fn config() -> crate::config::Config {
		crate::config::Config::from_toml_str("").expect("config")
	}

	#[tokio::test]
	async fn creates_log_retains_latest_fifteen_and_survives_reopening() {
		let directory = tempfile::tempdir().expect("directory");
		let path = directory.path().join(LOG_FILE_NAME);
		let log = AdminActionLog::new(path.clone());
		for _ in 0..16 {
			log.begin(&config(), 76561198000000001, AdminAction::Daemon,
				ActionChoice::SoftRestart, true).await.expect("append");
		}
		let reopened = AdminActionLog::new(path.clone());
		let entries = reopened.history(15).await.expect("history");
		assert_eq!(entries.len(), 15);
		assert_eq!(entries.first().expect("newest").id, 16);
		assert_eq!(entries.last().expect("oldest").id, 2);
		assert_eq!(entries[0].steam_id, "76561198000000001");
		assert_eq!(entries[0].action_id, 2);
		assert!(entries[0].modifiers["force"]);
		assert!(entries[0].unix_time > 0);
		#[cfg(unix)]
		{
			use std::os::unix::fs::PermissionsExt;
			assert_eq!(std::fs::metadata(&path).expect("metadata").permissions().mode() & 0o777, 0o600);
		}
	}

	#[tokio::test]
	async fn disabled_logging_does_not_create_or_modify_file() {
		let directory = tempfile::tempdir().expect("directory");
		let path = directory.path().join(LOG_FILE_NAME);
		let log = AdminActionLog::new(path.clone());
		let mut config = config();
		config.admin_action_logging = false;
		assert_eq!(log.begin(&config, 1, AdminAction::Backend, ActionChoice::Restart, false)
			.await.expect("disabled"), None);
		assert!(!path.exists());
		config.admin_action_logging = true;
		log.begin(&config, 1, AdminAction::Backend, ActionChoice::Restart, false).await.expect("entry");
		let original = std::fs::read(&path).expect("read");
		config.admin_action_logging = false;
		log.begin(&config, 2, AdminAction::Backend, ActionChoice::Restart, false).await.expect("disabled");
		assert_eq!(std::fs::read(&path).expect("read"), original);
	}

	#[tokio::test]
	async fn finalizes_results_and_never_reinserts_evicted_entries() {
		let directory = tempfile::tempdir().expect("directory");
		let log = AdminActionLog::new(directory.path().join(LOG_FILE_NAME));
		let mut config = config();
		config.admin_action_logging_max = 1;
		let first = log.begin(&config, 1, AdminAction::GameServer, ActionChoice::Start, false)
			.await.expect("first");
		log.finish(first, ActionOutcome::Completed, Some(json::json!({"service": "l4d2"})), None)
			.await.expect("completed");
		let entries = log.history(15).await.expect("history");
		assert_eq!(entries[0].outcome, ActionOutcome::Completed);
		assert_eq!(entries[0].additional_data.as_ref().expect("data")["service"], "l4d2");
		let second = log.begin(&config, 2, AdminAction::GameServer, ActionChoice::Stop, false)
			.await.expect("second");
		log.finish(first, ActionOutcome::Failed, None, Some("late result".to_string())).await.expect("evicted");
		log.finish(second, ActionOutcome::Failed, None, Some("service failed".to_string())).await.expect("failed");
		let entries = log.history(15).await.expect("history");
		assert_eq!(entries.len(), 1);
		assert_eq!(entries[0].steam_id, "2");
		assert_eq!(entries[0].error.as_deref(), Some("service failed"));
	}

	#[tokio::test]
	async fn corrupt_log_is_preserved_and_invalid_choices_are_rejected() {
		let directory = tempfile::tempdir().expect("directory");
		let path = directory.path().join(LOG_FILE_NAME);
		std::fs::write(&path, "corrupt history").expect("write");
		let log = AdminActionLog::new(path.clone());
		assert!(log.begin(&config(), 1, AdminAction::Backend, ActionChoice::Restart, false).await.is_err());
		assert_eq!(std::fs::read_to_string(&path).expect("read"), "corrupt history");
		assert!(log.begin(&config(), 1, AdminAction::Backend, ActionChoice::Stop, false).await.is_err());
	}

	#[tokio::test]
	async fn concurrent_actions_keep_unique_ids_and_retention_limit() {
		let directory = tempfile::tempdir().expect("directory");
		let log = std::sync::Arc::new(AdminActionLog::new(directory.path().join(LOG_FILE_NAME)));
		let mut tasks = Vec::new();
		for steam_id in 1..=20 {
			let log = log.clone();
			tasks.push(tokio::spawn(async move {
				log.begin(&config(), steam_id, AdminAction::GameServer, ActionChoice::Restart, false)
					.await.expect("append")
			}));
		}
		for task in tasks { task.await.expect("task"); }
		let entries = log.history(15).await.expect("history");
		assert_eq!(entries.len(), 15);
		assert_eq!(entries[0].id, 20);
		assert!(entries.windows(2).all(|pair| pair[0].id > pair[1].id));
	}
}
