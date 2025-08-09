// SPDX-License-Identifier: GPL-3.0-only

#[cfg(any(feature = "rest_steam", feature = "rest_call_for_sub"))]
use crate::config::Config;
#[cfg(feature = "rest_sqlite")]
use crate::databases_rest::mount_database_routes;
#[cfg(feature = "server_query")]
use crate::LiveServerInfo::{live_server_info, live_server_info_kether};
#[cfg(feature = "rest_steam")]
use crate::steam_rest::mount_steam_routes;
use rocket::{launch, routes, Build, Rocket};
use rocket_cors::{AllowedOrigins, CorsOptions};

/// Launches the Rocket web server with all configured routes and middleware
/// 
/// This function is the main entry point for the REST API server. It configures
/// the Rocket web framework with CORS settings, port configuration, and all
/// available routes based on the enabled features.
/// 
/// # Features
/// * `rest_steam` - Mounts Steam-related REST endpoints
/// * `rest_call_for_sub` - Mounts call-for-sub REST endpoints
/// * `rest_sqlite` - Mounts database REST endpoints and manages database connection pool
/// * `server_query` - Mounts LiveServerInfo REST endpoints
/// * `sat` - Mounts Satanixon-specific REST endpoints
/// 
/// # Configuration
/// * **Port**: 3001 (configured for NodeJS/React compatibility)
/// * **CORS**: Configured for localhost, kether.pl, and specific IP addresses
/// * **Database**: SQLite connection pool (if rest_sqlite feature enabled)
/// * **Config**: Loads and manages application configuration
/// 
/// # CORS Origins
/// * `http://localhost:3000` - Local Kether website development
/// * `http://localhost:80` - Web browser testing
/// * `https://kether.pl` - Production HTTPS
/// * `http://kether.pl` - Production HTTP (fallback)
/// * `http://54.36.179.182` - L4D2 server for sub requests
/// 
/// # Routes
/// * `/api` - Database REST endpoints (if rest_sqlite enabled)
/// * `/api/LiveServerInfo` - Server query endpoints (if server_query enabled)
/// * `/api/steam` - Steam REST endpoints (if rest_steam enabled)
/// * `/api/callForSub` - Call-for-sub endpoints (if rest_call_for_sub enabled)
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
#[launch]
pub fn rocket() -> Rocket<Build> {
	// Configure CORS
	let allowed_origins = AllowedOrigins::some_exact(&[
		"http://localhost:3000", // Local Kether website 'npm run start'
		"http://localhost:80",   // Web Browser testing the paths and api
		"https://kether.pl",
		"http://kether.pl", // Unencrypted HTTP shouldn't really happen, but allow it just in case... Just don't break the website when it happens
		"http://54.36.179.182", // L4D2 server calling for sub through Rest in Pawn
	]);

	let cors = CorsOptions {
		allowed_origins,
		..Default::default()
	}
	.to_cors()
	.unwrap();

	let mut rocket_build = rocket::build()
		.configure(rocket::Config::figment().merge(("port", 3001))) // NodeJS / React port
		.attach(cors.clone());

	#[cfg(any(feature = "rest_steam", feature = "rest_call_for_sub", feature = "rest_sqlite"))]
	{
		let config = Config::load().expect("Failed to load configuration");
		rocket_build = rocket_build.manage(config);
	}
	#[cfg(feature = "rest_sqlite")]
	{
		let db_pool = crate::db::database::establish_connection_pool();
		rocket_build = rocket_build.manage(db_pool);
		rocket_build = rocket_build.mount("/api", mount_database_routes());
	}
	#[cfg(feature = "server_query")]
	{
		rocket_build = rocket_build.mount("/api/LiveServerInfo", routes![live_server_info, live_server_info_kether]);
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

	rocket_build
}
