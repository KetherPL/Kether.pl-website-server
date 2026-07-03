// SPDX-License-Identifier: GPL-3.0-only

#[cfg(any(feature = "rest_steam", feature = "rest_call_for_sub", feature = "server_query", feature = "rest_json_db"))]
use crate::steam_bot::registry;
#[cfg(feature = "server_query")]
use crate::LiveServerInfo::{live_server_info, live_server_info_kether, live_server_info_kether2};
#[cfg(feature = "rest_steam")]
use crate::steam_rest::mount_steam_routes;
#[cfg(feature = "rest_json_db")]
use crate::json_cmds_binds_rest::mount_json_routes;
#[cfg(feature = "maps_bridge")]
use crate::maps_bridge::{mount_maps_bridge_routes, MapsBridgeState};
#[cfg(feature = "rest_json_db")]
use crate::json_storage::JsonDatabase;
use rocket::{fs::{FileServer, Options}, routes, Build, Rocket};
use rocket_cors::{AllowedOrigins, CorsOptions};

/// Launches the Rocket web server with all configured routes and middleware
/// 
/// This function is the main entry point for the REST API server. It configures
/// the Rocket web framework with CORS settings, port configuration, and all
/// available routes based on the enabled features.
/// 
/// # Features
/// * `rest_json_db` - Mounts JSON-based database REST endpoints (binds, commands, suggestions, voting)
/// * `rest_steam` - Mounts Steam-related REST endpoints
/// * `rest_call_for_sub` - Mounts call-for-sub REST endpoints
/// * `server_query` - Mounts LiveServerInfo REST endpoints
/// * `sat` - Mounts Satanixon-specific REST endpoints
/// * `fastdl` - Mounts FastDL file server for Source/GoldSrc game content
/// 
/// # Configuration
/// * **Port**: 3001 (configured for NodeJS/React compatibility)
/// * **CORS**: Configured for localhost, kether.pl, and specific IP addresses
/// * **JSON Database**: In-memory with file-based persistence (if rest_json_db feature enabled)
/// * **Config**: Loads and manages application configuration from KISS.ini
/// 
/// # CORS Origins
/// * `http://localhost:3000` - Local Kether website development
/// * `http://localhost:80` - Web browser testing
/// * `https://kether.pl` - Production HTTPS
/// * `http://kether.pl` - Production HTTP (fallback)
/// * `http://54.36.179.182` - L4D2 server for sub requests
/// 
/// # Routes
/// * `/api` - JSON-based database REST endpoints (if rest_json_db enabled)
///   - `/api/binds/*` - Binds CRUD operations with embedded voting
///   - `/api/commands/*` - Commands CRUD operations
///   - `/api/bind_suggestions/*` - Bind suggestions CRUD operations
///   - `/api/bind_votings/*` - Voting operations on binds
/// * `/api/LiveServerInfo` - Server query endpoints (if server_query enabled)
/// * `/api/steam` - Steam REST endpoints (if rest_steam enabled)
/// * `/api/callForSub` - Call-for-sub endpoints (if rest_call_for_sub enabled)
/// * `/fastdl` - FastDL file server for Source/GoldSrc content (if fastdl enabled)
/// * `/` - Satanixon-specific endpoints (if sat enabled)
/// 
/// # Example
/// ```rust
/// #[launch]
/// fn rocket() -> Rocket<Build> {
///     // This function is automatically called by Rocket
/// }
/// ```
/// 
/// # Returns
/// A configured Rocket instance ready to launch
pub fn build_rocket() -> Rocket<Build> {
	// Configure CORS
	let allowed_origins = AllowedOrigins::some_exact(&[
		"http://localhost:3000", // Local Kether website 'npm run start'
		"http://localhost:80",   // Web Browser testing the paths and api
		"https://kether.pl",
		"http://kether.pl", // Unencrypted HTTP shouldn't really happen, but allow it just in case... Just don't break the website when it happens
		"https://21370000.xyz",
		"http://21370000.xyz",
		"http://54.36.179.182", // L4D2 server (OVH) calling for sub through Rest in Pawn
		"http://104.245.245.137", // L4D2 server (POLANDVPN/host4fun) calling for sub through Rest in Pawn
	]);

	let cors = CorsOptions {
		allowed_origins,
		allow_credentials: true,
		..Default::default()
	}
	.to_cors()
	.unwrap();

	let mut rocket_build = rocket::build()
		.configure(rocket::Config::figment().merge(("port", 3001))) // NodeJS / React port
		.attach(cors.clone());

	#[cfg(feature = "rest_api")]
	{
		// Initialize the plan timestamp broadcaster
		crate::steam_bot::plan_broadcast::init_broadcaster();
	}

	#[cfg(any(feature = "rest_steam", feature = "rest_call_for_sub", feature = "rest_json_db", feature = "server_query", feature = "maps_bridge"))]
	{
		rocket_build = rocket_build.manage(registry::config_handle());
	}
	#[cfg(feature = "rest_json_db")]
	{
		// Load JSON database using smol runtime
		let json_db = smol::block_on(JsonDatabase::load())
			.expect("Failed to load JSON database");
		rocket_build = rocket_build.manage(json_db);
		rocket_build = rocket_build.mount("/api", mount_json_routes());
	}
	#[cfg(feature = "maps_bridge")]
	{
		let base_path = crate::config::exe_dir().expect("Failed to get executable directory");
		let config = registry::config_handle()
			.read()
			.expect("config lock poisoned")
			.clone();
		let maps_bridge = MapsBridgeState::from_config(&config, base_path)
			.expect("Failed to initialize maps bridge");
		rocket_build = rocket_build.manage(maps_bridge);
		rocket_build = rocket_build.mount("/api", mount_maps_bridge_routes());
	}
	#[cfg(feature = "auth")]
	{
		rocket_build = rocket_build
			.manage(crate::auth::ExchangeCodeStore::new())
			.mount("/api/auth", crate::auth::mount_auth_routes());
	}
	#[cfg(feature = "server_query")]
	{
		rocket_build = rocket_build.mount("/api/LiveServerInfo", routes![
			live_server_info, live_server_info_kether, live_server_info_kether2,
		]);
	}
	#[cfg(feature = "rest_steam")]
	{
		rocket_build = rocket_build.mount("/api/steam", mount_steam_routes());
	}
	#[cfg(feature = "rest_call_for_sub")]
	{
		rocket_build = rocket_build.mount("/api/callForSub", crate::call_for_sub_rest::mount_callForSub_routes());
	}
	#[cfg(feature = "sat")]
	{
		rocket_build = rocket_build.mount("/", crate::sat_specific_rest::mount_sat_specific_routes());
		rocket_build = rocket_build.register("/", crate::sat_specific_rest::mount_sat_specific_catchers());
	}
	#[cfg(feature = "fastdl")]
	{
		// Attach FastDL cache headers fairing
		rocket_build = rocket_build.attach(crate::fastdl_rest::FastDLCacheHeaders);
		
		// Mount FileServer first (rank 0 - highest priority) for file serving
		let options = Options::Missing | Options::NormalizeDirs;
		rocket_build = rocket_build.mount("/fastdl", FileServer::new("./fastdl", options));
		
		// Register FastDL catcher for directory listings (handles 404s from FileServer)
		rocket_build = rocket_build.register("/fastdl", crate::fastdl_rest::mount_fastdl_catchers());
	}

	#[cfg(feature = "rest_api")]
	{
		// Mount WebSocket routes for plan timestamp streaming
		rocket_build = rocket_build.mount("/api/ws", crate::plan_ws::mount_plan_ws_routes());
	}

	rocket_build
}

/// Runs the Rocket server until `shutdown` receives a signal.
pub async fn run(mut shutdown: tokio::sync::broadcast::Receiver<()>) -> Result<(), String> {
	let rocket = build_rocket();
	let rocket = rocket.ignite().await.map_err(|e| e.to_string())?;
	let shutdown_handle = rocket.shutdown();

	tokio::spawn(async move {
		let _ = shutdown.recv().await;
		shutdown_handle.notify();
	});

	rocket.launch().await.map_err(|e| e.to_string())?;
	Ok(())
}
