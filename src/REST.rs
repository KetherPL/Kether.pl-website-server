// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use crate::databases_rest::mount_database_routes;
use crate::LiveServerInfo::{live_server_info, live_server_info_kether};
use crate::steam_rest::mount_steam_routes;
use rocket::{launch, routes};
use rocket_cors::{AllowedOrigins, CorsOptions};

#[launch]
pub fn rocket() -> _ {
	// Configure CORS
	let allowed_origins = AllowedOrigins::some_exact(&[
		"http://localhost:3000", // Local Kether website 'npm run start'
		"http://localhost:80",   // Web Browser testing the paths and api
		"https://kether.pl",
		"http://kether.pl", // Unencrypted HTTP shouldn't really happen, but allow it just in case... Just don't break the website when it happens
	]);

	let cors = CorsOptions {
		allowed_origins,
		..Default::default()
	}
	.to_cors()
	.unwrap();

	let config = Config::load().expect("Failed to load configuration");
	let db_pool = crate::db::database::establish_connection_pool();

	rocket::build()
		.manage(config)
		.manage(db_pool)
		.configure(rocket::Config::figment().merge(("port", 3001))) // NodeJS / React port
		.mount("/api/LiveServerInfo", routes![live_server_info, live_server_info_kether])
		.mount("/api", mount_database_routes())
		.mount("/api/steam", mount_steam_routes()) // Mount the new steam routes
		.attach(cors)
}
