// SPDX-License-Identifier: GPL-3.0-only

use rocket::{
	http::Status,
	serde::json::Json,
	Route, State,
};
use rocket::{options, post, routes};
use rocket::serde::{Deserialize, Serialize};

use crate::config::Config;
use crate::json_api::{binds, commands, suggestions, utils::ok_status, votings};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct VerifyAdminRequest {
	pub steam_id: i64,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct VerifyAdminResponse {
	pub is_admin: bool,
}

#[post("/admin/verify", data = "<request>")]
pub fn verify_admin(
	config: &State<Config>,
	request: Json<VerifyAdminRequest>,
) -> Result<Json<VerifyAdminResponse>, Status> {
	let is_admin = config.is_admin(request.steam_id);
	Ok(Json(VerifyAdminResponse { is_admin }))
}

#[options("/admin/verify")]
pub fn options_verify_admin() -> Status {
	ok_status()
}

pub fn mount_json_routes() -> Vec<Route> {
	let mut all_routes: Vec<Route> = Vec::new();
	all_routes.extend(binds::routes());
	all_routes.extend(suggestions::routes());
	all_routes.extend(commands::routes());
	all_routes.extend(votings::routes());
	all_routes.extend(routes![verify_admin, options_verify_admin]);
	all_routes
}

