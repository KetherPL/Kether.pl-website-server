// SPDX-License-Identifier: GPL-3.0-only

//! Admin-only maintenance API (`/api/admin`): service status and restart buttons for the
//! website backend itself and the KetherServerDaemon.

use std::time::Duration;

use chrono::{DateTime, Utc};
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, options, post, routes, Route, State};

use crate::auth::AdminUser;
use crate::config::Config;
use crate::control;
use crate::steam_bot::registry::ConfigHandle;

const DAEMON_TIMEOUT: Duration = Duration::from_secs(4);

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct BackendStatus {
	pub version: &'static str,
	pub pid: u32,
	pub process_started_at: DateTime<Utc>,
	pub services_started_at: DateTime<Utc>,
	pub uptime_secs: u64,
	pub restart_supported: bool,
}

/// Mirrors the daemon's `GET /api/daemon/status` payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct DaemonStatus {
	pub version: String,
	pub pid: u32,
	pub started_at: DateTime<Utc>,
	pub uptime_secs: u64,
	pub busy: Option<String>,
	pub restart_supported: bool,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct DaemonReport {
	/// Whether `server_daemon.daemon_url` is set.
	pub configured: bool,
	/// Whether the daemon answered.
	pub reachable: bool,
	/// Full status, when the daemon supports `/api/daemon/status`.
	pub status: Option<DaemonStatus>,
	/// Why the status is missing, if it is.
	pub error: Option<String>,
}

/// Mirrors the daemon's game server status payload.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct GameServerStatus {
	pub service: String,
	pub active_state: String,
	pub active_state_raw: String,
	pub sub_state: String,
	pub description: String,
	pub main_pid: u32,
	pub main_comm: Option<String>,
	pub main_started_at: Option<DateTime<Utc>>,
	pub main_uptime_secs: Option<u64>,
	pub srcds_pid: Option<u32>,
	pub srcds_comm: Option<String>,
	pub srcds_started_at: Option<DateTime<Utc>>,
	pub srcds_uptime_secs: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct GameServerActionResult {
	pub action: String,
	pub status: GameServerStatus,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminStatus {
	pub backend: BackendStatus,
	pub daemon: DaemonReport,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct RestartAccepted {
	pub requested_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminErrorResponse {
	pub error: String,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct DaemonEnvelope<T> {
	#[allow(dead_code)]
	success: bool,
	data: Option<T>,
	error: Option<String>,
}

type AdminResult<T> = Result<Json<T>, (Status, Json<AdminErrorResponse>)>;

fn admin_error(status: Status, error: impl Into<String>) -> (Status, Json<AdminErrorResponse>) {
	(status, Json(AdminErrorResponse { error: error.into() }))
}

fn config_snapshot(handle: &State<ConfigHandle>) -> std::sync::Arc<Config> {
	match handle.read() {
		Ok(guard) => guard.clone(),
		Err(e) => e.into_inner().clone(),
	}
}

fn daemon_base_url(config: &Config) -> Option<String> {
	let url = config.server_daemon_url.trim().trim_end_matches('/');
	(!url.is_empty()).then(|| url.to_string())
}

fn daemon_request(
	client: &reqwest::Client,
	method: reqwest::Method,
	url: String,
	config: &Config,
) -> reqwest::RequestBuilder {
	let request = client.request(method, url).timeout(DAEMON_TIMEOUT);
	match config.server_daemon_sync_api_key.trim() {
		"" => request,
		key => request.bearer_auth(key),
	}
}

async fn fetch_daemon_report(config: &Config) -> DaemonReport {
	let Some(base) = daemon_base_url(config) else {
		return DaemonReport {
			configured: false,
			reachable: false,
			status: None,
			error: Some("server_daemon.daemon_url is not configured".to_string()),
		};
	};
	let client = reqwest::Client::new();

	match daemon_request(&client, reqwest::Method::GET, format!("{base}/api/daemon/status"), config)
		.send()
		.await
	{
		Ok(response) if response.status().is_success() => {
			match response.json::<DaemonEnvelope<DaemonStatus>>().await {
				Ok(envelope) => DaemonReport {
					configured: true,
					reachable: true,
					error: envelope.error,
					status: envelope.data,
				},
				Err(e) => DaemonReport {
					configured: true,
					reachable: true,
					status: None,
					error: Some(format!("Invalid daemon status response: {e}")),
				},
			}
		}
		Ok(response) if response.status() == reqwest::StatusCode::NOT_FOUND => {
			// Older daemon without lifecycle endpoints: report whether it is alive at all.
			let alive = daemon_request(&client, reqwest::Method::GET, format!("{base}/health"), config)
				.send()
				.await
				.is_ok_and(|r| r.status().is_success());
			DaemonReport {
				configured: true,
				reachable: alive,
				status: None,
				error: Some("Daemon is too old to report status or restart remotely".to_string()),
			}
		}
		Ok(response) => DaemonReport {
			configured: true,
			reachable: true,
			status: None,
			error: Some(format!("Daemon status request failed: HTTP {}", response.status())),
		},
		Err(e) => DaemonReport {
			configured: true,
			reachable: false,
			status: None,
			error: Some(format!("Daemon unreachable: {e}")),
		},
	}
}

#[get("/status")]
pub async fn admin_status(_admin: AdminUser, config_handle: &State<ConfigHandle>) -> Json<AdminStatus> {
	let config = config_snapshot(config_handle);
	let (services_started_at, uptime_secs) = control::services_started();
	Json(AdminStatus {
		backend: BackendStatus {
			version: env!("CARGO_PKG_VERSION"),
			pid: std::process::id(),
			process_started_at: control::process_started_at(),
			services_started_at,
			uptime_secs,
			restart_supported: control::restart_supported(),
		},
		daemon: fetch_daemon_report(&config).await,
	})
}

/// Restarts the REST server and SteamBot in-process (same as the REPL `restart` command).
#[post("/restart/backend")]
pub fn admin_restart_backend(admin: AdminUser) -> AdminResult<RestartAccepted> {
	control::request_restart().map_err(|e| admin_error(Status::ServiceUnavailable, e))?;
	println!("Admin {} requested a website backend restart", admin.0);
	Ok(Json(RestartAccepted { requested_at: Utc::now() }))
}

/// Asks the KetherServerDaemon to restart itself. Without `force`, the daemon refuses
/// (409) while map downloads/installs/updates are running.
#[post("/restart/daemon?<force>")]
pub async fn admin_restart_daemon(
	admin: AdminUser,
	config_handle: &State<ConfigHandle>,
	force: Option<bool>,
) -> AdminResult<RestartAccepted> {
	let config = config_snapshot(config_handle);
	let base = daemon_base_url(&config).ok_or_else(|| {
		admin_error(Status::ServiceUnavailable, "server_daemon.daemon_url is not configured")
	})?;
	let force = force.unwrap_or(false);
	let client = reqwest::Client::new();
	let response = daemon_request(
		&client,
		reqwest::Method::POST,
		format!("{base}/api/daemon/restart?force={force}"),
		&config,
	)
	.send()
	.await
	.map_err(|e| admin_error(Status::BadGateway, format!("Daemon unreachable: {e}")))?;

	let status = response.status();
	if status.is_success() {
		println!("Admin {} requested a daemon restart (force: {})", admin.0, force);
		return Ok(Json(RestartAccepted { requested_at: Utc::now() }));
	}
	let message = response
		.json::<DaemonEnvelope<rocket::serde::json::Value>>()
		.await
		.ok()
		.and_then(|envelope| envelope.error)
		.unwrap_or_else(|| format!("Daemon refused the restart: HTTP {status}"));
	Err(match status {
		reqwest::StatusCode::CONFLICT => admin_error(Status::Conflict, message),
		reqwest::StatusCode::NOT_FOUND => admin_error(
			Status::NotImplemented,
			"Daemon is too old to restart remotely; update it first",
		),
		_ => admin_error(Status::BadGateway, message),
	})
}

#[options("/status")]
pub fn options_admin_status() -> Status {
	Status::Ok
}

#[options("/restart/backend")]
pub fn options_admin_restart_backend() -> Status {
	Status::Ok
}

#[options("/restart/daemon")]
pub fn options_admin_restart_daemon() -> Status {
	Status::Ok
}

#[get("/game-server/status")]
pub async fn admin_game_server_status(
	_admin: AdminUser,
	config_handle: &State<ConfigHandle>,
) -> AdminResult<GameServerStatus> {
	let config = config_snapshot(config_handle);
	let base = daemon_base_url(&config).ok_or_else(|| {
		admin_error(Status::ServiceUnavailable, "server_daemon.daemon_url is not configured")
	})?;
	let client = reqwest::Client::new();
	let response = daemon_request(
		&client,
		reqwest::Method::GET,
		format!("{base}/api/game-server/status"),
		&config,
	)
	.send()
	.await
	.map_err(|e| admin_error(Status::BadGateway, format!("Daemon unreachable: {e}")))?;

	if response.status().is_success() {
		let envelope = response
			.json::<DaemonEnvelope<GameServerStatus>>()
			.await
			.map_err(|e| admin_error(Status::BadGateway, format!("Invalid response: {e}")))?;
		let status = envelope
			.data
			.ok_or_else(|| admin_error(Status::BadGateway, "Daemon returned no data"))?;
		return Ok(Json(status));
	}
	let http_status = response.status();
	Err(admin_error(
		Status::BadGateway,
		format!("Daemon returned HTTP {http_status}"),
	))
}

async fn proxy_game_server_action(
	config: &Config,
	action: &str,
) -> AdminResult<GameServerActionResult> {
	let base = daemon_base_url(config).ok_or_else(|| {
		admin_error(Status::ServiceUnavailable, "server_daemon.daemon_url is not configured")
	})?;
	let client = reqwest::Client::new();
	let response = daemon_request(
		&client,
		reqwest::Method::POST,
		format!("{base}/api/game-server/{action}"),
		config,
	)
	.send()
	.await
	.map_err(|e| admin_error(Status::BadGateway, format!("Daemon unreachable: {e}")))?;

	if response.status().is_success() {
		let envelope = response
			.json::<DaemonEnvelope<GameServerActionResult>>()
			.await
			.map_err(|e| admin_error(Status::BadGateway, format!("Invalid response: {e}")))?;
		let result = envelope
			.data
			.ok_or_else(|| admin_error(Status::BadGateway, "Daemon returned no data"))?;
		return Ok(Json(result));
	}
	let http_status = response.status();
	let message = response
		.json::<DaemonEnvelope<rocket::serde::json::Value>>()
		.await
		.ok()
		.and_then(|e| e.error)
		.unwrap_or_else(|| format!("Daemon returned HTTP {http_status}"));
	Err(admin_error(Status::BadGateway, message))
}

#[post("/game-server/start")]
pub async fn admin_game_server_start(
	admin: AdminUser,
	config_handle: &State<ConfigHandle>,
) -> AdminResult<GameServerActionResult> {
	let config = config_snapshot(config_handle);
	println!("Admin {} requested L4D2 server start", admin.0);
	proxy_game_server_action(&config, "start").await
}

#[post("/game-server/stop")]
pub async fn admin_game_server_stop(
	admin: AdminUser,
	config_handle: &State<ConfigHandle>,
) -> AdminResult<GameServerActionResult> {
	let config = config_snapshot(config_handle);
	println!("Admin {} requested L4D2 server stop", admin.0);
	proxy_game_server_action(&config, "stop").await
}

#[post("/game-server/restart")]
pub async fn admin_game_server_restart(
	admin: AdminUser,
	config_handle: &State<ConfigHandle>,
) -> AdminResult<GameServerActionResult> {
	let config = config_snapshot(config_handle);
	println!("Admin {} requested L4D2 server restart", admin.0);
	proxy_game_server_action(&config, "restart").await
}

#[options("/game-server/status")]
pub fn options_game_server_status() -> Status {
	Status::Ok
}

#[options("/game-server/start")]
pub fn options_game_server_start() -> Status {
	Status::Ok
}

#[options("/game-server/stop")]
pub fn options_game_server_stop() -> Status {
	Status::Ok
}

#[options("/game-server/restart")]
pub fn options_game_server_restart() -> Status {
	Status::Ok
}

pub fn mount_admin_routes() -> Vec<Route> {
	routes![
		admin_status,
		admin_restart_backend,
		admin_restart_daemon,
		admin_game_server_status,
		admin_game_server_start,
		admin_game_server_stop,
		admin_game_server_restart,
		options_admin_status,
		options_admin_restart_backend,
		options_admin_restart_daemon,
		options_game_server_status,
		options_game_server_start,
		options_game_server_stop,
		options_game_server_restart,
	]
}

#[cfg(test)]
mod tests {
	use super::*;

	fn config_with_daemon(url: &str) -> Config {
		Config::from_toml_str(&format!(
			r#"
frontend_admins = []
[server_daemon]
daemon_url = "{url}"
[steam.chat]
group_id = 1
chat_id = 2
"#
		))
		.expect("config")
	}

	#[test]
	fn daemon_base_url_trims_and_detects_missing() {
		assert_eq!(
			daemon_base_url(&config_with_daemon("http://127.0.0.1:8080/ ")).as_deref(),
			Some("http://127.0.0.1:8080")
		);
		assert_eq!(daemon_base_url(&config_with_daemon("  ")), None);
	}

	const ADMIN_ID: i64 = 76561198000000001;
	const USER_ID: i64 = 76561198000000002;

	fn auth_test_config() -> Config {
		Config::from_toml_str(&format!(
			r#"
frontend_admins = [{ADMIN_ID}]
[auth]
session_secret = "test-secret-that-is-long-enough-for-hs256"
csrf_allowed_origins = ["https://kether.pl"]
[server_daemon]
daemon_url = ""
[steam.chat]
group_id = 1
chat_id = 2
"#
		))
		.expect("config")
	}

	async fn client() -> (rocket::local::asynchronous::Client, Config) {
		let config = auth_test_config();
		let handle: ConfigHandle = std::sync::Arc::new(std::sync::RwLock::new(std::sync::Arc::new(
			config.clone(),
		)));
		let rocket = rocket::build().manage(handle).mount("/api/admin", mount_admin_routes());
		(
			rocket::local::asynchronous::Client::tracked(rocket).await.expect("client"),
			config,
		)
	}

	fn bearer(steam_id: i64, config: &Config) -> rocket::http::Header<'static> {
		let token = crate::auth::mint_session(steam_id, config).expect("token");
		rocket::http::Header::new("Authorization", format!("Bearer {token}"))
	}

	#[rocket::async_test]
	async fn status_requires_admin() {
		let (client, config) = client().await;
		let anonymous = client.get("/api/admin/status").dispatch().await;
		assert_eq!(anonymous.status(), Status::Unauthorized);

		let user = client
			.get("/api/admin/status")
			.header(bearer(USER_ID, &config))
			.dispatch()
			.await;
		assert_eq!(user.status(), Status::Forbidden);

		let admin = client
			.get("/api/admin/status")
			.header(bearer(ADMIN_ID, &config))
			.dispatch()
			.await;
		assert_eq!(admin.status(), Status::Ok);
		let body: rocket::serde::json::Value = admin.into_json().await.expect("json");
		assert_eq!(body["backend"]["pid"], std::process::id());
		assert_eq!(body["daemon"]["configured"], false);
	}

	fn origin() -> rocket::http::Header<'static> {
		rocket::http::Header::new("Origin", "https://kether.pl")
	}

	#[rocket::async_test]
	async fn restarts_require_allowed_origin() {
		let (client, config) = client().await;
		let foreign = client
			.post("/api/admin/restart/backend")
			.header(bearer(ADMIN_ID, &config))
			.header(rocket::http::Header::new("Origin", "https://evil.example"))
			.dispatch()
			.await;
		assert_eq!(foreign.status(), Status::Forbidden);
	}

	#[rocket::async_test]
	async fn restarts_require_admin_and_report_unavailable_targets() {
		let (client, config) = client().await;
		let user = client
			.post("/api/admin/restart/backend")
			.header(bearer(USER_ID, &config))
			.header(origin())
			.dispatch()
			.await;
		assert_eq!(user.status(), Status::Forbidden);

		// Tests never register the REPL command channel, so a backend restart is unavailable.
		let backend = client
			.post("/api/admin/restart/backend")
			.header(bearer(ADMIN_ID, &config))
			.header(origin())
			.dispatch()
			.await;
		assert_eq!(backend.status(), Status::ServiceUnavailable);

		let daemon = client
			.post("/api/admin/restart/daemon?force=true")
			.header(bearer(ADMIN_ID, &config))
			.header(origin())
			.dispatch()
			.await;
		assert_eq!(daemon.status(), Status::ServiceUnavailable);
	}

	#[tokio::test]
	async fn daemon_report_when_not_configured() {
		let report = fetch_daemon_report(&config_with_daemon("")).await;
		assert!(!report.configured);
		assert!(!report.reachable);
		assert!(report.error.is_some());
	}

	#[tokio::test]
	async fn daemon_report_when_unreachable() {
		// Port 9 (discard) on localhost is not served in test environments.
		let report = fetch_daemon_report(&config_with_daemon("http://127.0.0.1:9")).await;
		assert!(report.configured);
		assert!(!report.reachable);
		assert!(report.error.unwrap().starts_with("Daemon unreachable"));
	}

	#[rocket::async_test]
	async fn game_server_endpoints_require_admin_and_origin() {
		let (client, config) = client().await;
		let anon = client.get("/api/admin/game-server/status").dispatch().await;
		assert_eq!(anon.status(), Status::Unauthorized);

		let user = client
			.get("/api/admin/game-server/status")
			.header(bearer(USER_ID, &config))
			.dispatch()
			.await;
		assert_eq!(user.status(), Status::Forbidden);

		let user_post = client
			.post("/api/admin/game-server/start")
			.header(bearer(USER_ID, &config))
			.header(origin())
			.dispatch()
			.await;
		assert_eq!(user_post.status(), Status::Forbidden);

		let foreign_post = client
			.post("/api/admin/game-server/start")
			.header(bearer(ADMIN_ID, &config))
			.header(rocket::http::Header::new("Origin", "https://evil.example"))
			.dispatch()
			.await;
		assert_eq!(foreign_post.status(), Status::Forbidden);

		// When daemon is not configured, admin gets ServiceUnavailable:
		let admin_status = client
			.get("/api/admin/game-server/status")
			.header(bearer(ADMIN_ID, &config))
			.dispatch()
			.await;
		assert_eq!(admin_status.status(), Status::ServiceUnavailable);

		let admin_start = client
			.post("/api/admin/game-server/start")
			.header(bearer(ADMIN_ID, &config))
			.header(origin())
			.dispatch()
			.await;
		assert_eq!(admin_start.status(), Status::ServiceUnavailable);
	}
}
