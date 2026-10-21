// SPDX-License-Identifier: GPL-3.0-only

//! Process lifecycle state shared between `main` and the admin REST API: start times and
//! a way for HTTP handlers to request the same in-process restart the REPL `restart`
//! command performs (REST server + SteamBot are stopped and started again).

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use tokio::sync::mpsc::UnboundedSender;

use crate::repl::DaemonCommand;

/// Delay between answering a restart request and stopping the services, so the HTTP
/// response reaches the browser first.
const RESTART_RESPONSE_GRACE: Duration = Duration::from_millis(500);

static PROCESS_STARTED: OnceLock<DateTime<Utc>> = OnceLock::new();
static SERVICES_STARTED: Mutex<Option<(Instant, DateTime<Utc>)>> = Mutex::new(None);
static COMMAND_TX: OnceLock<UnboundedSender<DaemonCommand>> = OnceLock::new();

/// Records the process start time. Call once at the beginning of `main`.
pub fn mark_process_started() {
	let _ = PROCESS_STARTED.set(Utc::now());
}

/// Records that the services (REST + SteamBot) were (re)started.
pub fn mark_services_started() {
	if let Ok(mut started) = SERVICES_STARTED.lock() {
		*started = Some((Instant::now(), Utc::now()));
	}
}

/// When the process started (not reset by in-process restarts).
pub fn process_started_at() -> DateTime<Utc> {
	PROCESS_STARTED.get().copied().unwrap_or_else(Utc::now)
}

/// When the services were last (re)started, and seconds since then.
pub fn services_started() -> (DateTime<Utc>, u64) {
	match SERVICES_STARTED.lock().ok().and_then(|started| *started) {
		Some((instant, at)) => (at, instant.elapsed().as_secs()),
		None => (process_started_at(), 0),
	}
}

/// Gives the admin API access to the daemon command channel used by the REPL.
pub fn set_command_sender(command_tx: UnboundedSender<DaemonCommand>) {
	let _ = COMMAND_TX.set(command_tx);
}

/// Whether restart requests can be served (the process runs with `--service`).
pub fn restart_supported() -> bool {
	COMMAND_TX.get().is_some()
}

/// Schedules an in-process restart of the REST server and SteamBot.
pub fn request_restart() -> Result<(), String> {
	let command_tx = COMMAND_TX
		.get()
		.cloned()
		.ok_or_else(|| "Restart is not available (not running with --service)".to_string())?;
	tokio::spawn(async move {
		tokio::time::sleep(RESTART_RESPONSE_GRACE).await;
		if let Err(e) = command_tx.send(DaemonCommand::Restart) {
			eprintln!("Failed to request restart: {}", e);
		}
	});
	Ok(())
}
