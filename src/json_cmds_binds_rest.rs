// SPDX-License-Identifier: GPL-3.0-only

use rocket::Route;

use crate::json_api::{binds, commands, suggestions, votings};

pub fn mount_json_routes() -> Vec<Route> {
	let mut all_routes: Vec<Route> = Vec::new();
	all_routes.extend(binds::routes());
	all_routes.extend(suggestions::routes());
	all_routes.extend(commands::routes());
	all_routes.extend(votings::routes());
	all_routes
}
