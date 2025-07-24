// SPDX-License-Identifier: GPL-3.0-only

#[cfg(feature = "rest_call_for_sub")]
use std::sync::Arc;

#[cfg(any(feature = "rest_steam", feature = "rest_call_for_sub"))]
use crate::config::Config;
#[cfg(any(feature = "rest_steam", feature = "rest_call_for_sub"))]
use edon::{self, Nodejs};
#[cfg(feature = "rest_sqlite")]
use crate::databases_rest::mount_database_routes;
#[cfg(feature = "server_query")]
use crate::LiveServerInfo::{live_server_info, live_server_info_kether};
#[cfg(feature = "rest_steam")]
use crate::steam_rest::mount_steam_routes;
use rocket::{routes, Build, Rocket};
use rocket_cors::{AllowedOrigins, CorsOptions};

// Add this function to accept Node.js instance
#[cfg(feature = "rest_call_for_sub")]
pub async fn main_with_nodejs(nodejs_instance: Arc<Nodejs>) -> Result<(), rocket::Error> {
	let rocket = rocket(Some(nodejs_instance));
	rocket.launch().await?;
	Ok(())
}

// Regular main function for when Node.js is not needed
pub async fn _main() -> Result<(), rocket::Error> {
	let rocket = rocket(None);
	rocket.launch().await?;
	Ok(())
}

//#[launch]
pub fn rocket(nodejs_instance: Option<Arc<Nodejs>>) -> Rocket<Build> {
	// Configure CORS
	let allowed_origins = AllowedOrigins::some_exact(&[
		"http://localhost:3000", // Local Kether website 'npm run start'
		"http://localhost:80",   // Web Browser testing the paths and api
		"https://kether.pl",
		"http://kether.pl", // Unencrypted HTTP shouldn't really happen, but allow it just in case... Just don't break the website when it happens
		"http://54.36.179.182", // L4D2 server calling for sub through Rest in Pawn
	]);

	let cors = CorsOptions {
		// allowed_origins,
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
		if let Some(node) = nodejs_instance {
			rocket_build = rocket_build.manage(node);
		}
	}
	#[cfg(feature = "sat")]
	{
		rocket_build = rocket_build.mount("/", crate::sat_specific_rest::mount_sat_specific_routes());
	}

	rocket_build
}
