// SPDX-License-Identifier: GPL-3.0-only

//! Process lifecycle state shared between `main` and the admin REST API: start times and
//! a way for HTTP handlers to request the same in-process soft restart the REPL `soft-restart`
//! command performs (REST server + SteamBot are stopped and started again).

use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use chrono::{DateTime, Utc};
use tokio::sync::mpsc::UnboundedSender;

use crate::repl::DaemonCommand;

/// Delay between answering a restart request and stopping the services, so the HTTP
/// response reaches the browser first.
const SOFT_RESTART_RESPONSE_GRACE: Duration = Duration::from_millis(500);

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

/// Whether soft-restart requests can be served (the process runs with `--service`).
pub fn soft_restart_supported() -> bool {
	COMMAND_TX.get().is_some()
}

/// Schedules an in-process soft restart of the REST server and SteamBot.
pub fn request_soft_restart() -> Result<(), String> {
	let command_tx = COMMAND_TX
		.get()
		.cloned()
		.ok_or_else(|| "Soft restart is not available (not running with --service)".to_string())?;
	tokio::spawn(async move {
		tokio::time::sleep(SOFT_RESTART_RESPONSE_GRACE).await;
		if let Err(e) = command_tx.send(DaemonCommand::SoftRestart) {
			eprintln!("Failed to request soft restart: {}", e);
		}
	});
	Ok(())
}

/// The systemd user service that runs the website backend.
pub const BACKEND_SERVICE: &str = "kiss.service";
const SERVICE_RESTART_DELAY: &str = "--on-active=1s";

fn service_restart_command(uid: u32) -> std::process::Command {
	// The user manager owns the timer, so stopping kiss.service cannot kill it.
	let mut command = std::process::Command::new("systemd-run");
	command.args([
		"--user",
		"--collect",
		SERVICE_RESTART_DELAY,
		"--timer-property=AccuracySec=100ms",
		"--setenv=XDG_RUNTIME_DIR",
		"--setenv=DBUS_SESSION_BUS_ADDRESS",
		"--",
		"systemctl",
		"--user",
		"restart",
		BACKEND_SERVICE,
	]);
	command.env("XDG_RUNTIME_DIR", format!("/run/user/{uid}"));
	command.env("DBUS_SESSION_BUS_ADDRESS", format!("unix:path=/run/user/{uid}/bus"));
	command
}

/// Whether this platform supports systemd user-service restarts.
pub fn restart_supported() -> bool {
	cfg!(target_os = "linux")
}

/// Schedule a restart and return once the user manager accepts the timer.
pub fn request_restart() -> Result<(), String> {
	#[cfg(target_os = "linux")]
	{
		let uid = unsafe { libc::getuid() };
		run_service_restart_command(&mut service_restart_command(uid))?;
		println!("Website backend service restart scheduled ({BACKEND_SERVICE})");
		Ok(())
	}
	#[cfg(not(target_os = "linux"))]
	{
		Err("Systemd user-service restart is only available on Linux".to_string())
	}
}

fn run_service_restart_command(command: &mut std::process::Command) -> Result<(), String> {
	let output = command.output()
		.map_err(|error| format!("Failed to schedule {BACKEND_SERVICE} restart: {error}"))?;
	if !output.status.success() {
		return Err(format!(
			"Failed to schedule {BACKEND_SERVICE} restart ({}): {} {}",
			output.status,
			String::from_utf8_lossy(&output.stderr),
			String::from_utf8_lossy(&output.stdout),
		));
	}
	Ok(())
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn service_restart_targets_backend_and_forwards_user_manager_environment() {
		let command = service_restart_command(1234);
		assert_eq!(command.get_program(), "systemd-run");
		assert_eq!(command.get_args().collect::<Vec<_>>(), [
			"--user", "--collect", "--on-active=1s",
			"--timer-property=AccuracySec=100ms",
			"--setenv=XDG_RUNTIME_DIR", "--setenv=DBUS_SESSION_BUS_ADDRESS",
			"--", "systemctl", "--user", "restart", "kiss.service",
		].map(std::ffi::OsStr::new));
		let environment = command.get_envs().collect::<std::collections::HashMap<_, _>>();
		assert_eq!(environment[std::ffi::OsStr::new("XDG_RUNTIME_DIR")],
			Some(std::ffi::OsStr::new("/run/user/1234")));
		assert_eq!(environment[std::ffi::OsStr::new("DBUS_SESSION_BUS_ADDRESS")],
			Some(std::ffi::OsStr::new("unix:path=/run/user/1234/bus")));
	}

	#[test]
	fn service_restart_reports_missing_scheduler() {
		let directory = tempfile::tempdir().expect("tempdir");
		let mut command = std::process::Command::new(directory.path().join("missing-systemd-run"));
		let error = run_service_restart_command(&mut command).expect_err("missing scheduler");
		assert!(error.contains("Failed to schedule kiss.service restart"));
	}

	#[cfg(unix)]
	#[test]
	fn service_restart_reports_manager_rejection() {
		let mut command = std::process::Command::new("sh");
		command.args(["-c", "echo 'manager rejected restart' >&2; exit 1"]);
		let error = run_service_restart_command(&mut command).expect_err("manager rejected restart");
		assert!(error.contains("kiss.service"));
		assert!(error.contains("manager rejected restart"));
	}
}
