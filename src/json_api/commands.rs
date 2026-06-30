// SPDX-License-Identifier: GPL-3.0-only

use rocket::{
	get, options, post, put, routes,
	http::Status,
	serde::json::Json,
	Route, State,
};
use rocket::serde::{Deserialize, Serialize};

use crate::json_api::models::Command;
use crate::json_api::utils::{ok_status, storage_error_status};
use crate::json_storage::JsonDatabase;
use crate::auth::AdminUser;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct CreateCommandRequest {
	pub command: String,
	pub description: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DeleteCommandRequest {
	pub id: i32,
}

#[get("/commands/<command_id>")]
pub fn get_command(db: &State<JsonDatabase>, command_id: i32) -> Result<Json<Command>, Status> {
	db.get_command(command_id)
		.map(Json)
		.map_err(|e| storage_error_status("getting command", &e, &["not found"], &[]))
}

#[get("/commands/getCommands")]
pub fn list_commands(db: &State<JsonDatabase>) -> Result<Json<Vec<Command>>, Status> {
	db.get_all_commands()
		.map(Json)
		.map_err(|e| storage_error_status("listing commands", &e, &[], &[]))
}

#[post("/commands/addCommand", data = "<request>")]
pub async fn create_command(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	request: Json<CreateCommandRequest>,
) -> Result<Json<Command>, Status> {
	db.create_command(request.command.clone(), request.description.clone()).await
		.map(Json)
		.map_err(|e| storage_error_status("creating command", &e, &[], &["already exists"]))
}

#[post("/commands/deleteCommand", data = "<request>")]
pub async fn delete_command(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	request: Json<DeleteCommandRequest>,
) -> Result<Json<usize>, Status> {
	db.delete_command(request.id).await
		.map(|_| Json(1))
		.map_err(|e| storage_error_status("deleting command", &e, &["not found"], &[]))
}

#[post("/commands/updateCommand", data = "<command_to_update>")]
pub async fn update_command(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	command_to_update: Json<Command>,
) -> Result<Json<Command>, Status> {
	db.update_command(
		command_to_update.id,
		command_to_update.command.clone(),
		command_to_update.description.clone(),
	).await
	.map(Json)
	.map_err(|e| storage_error_status("updating command", &e, &["not found"], &["already exists"]))
}

#[put("/commands/<command_id>", data = "<command_to_update>")]
pub async fn update_command_by_id(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	command_id: i32,
	command_to_update: Json<Command>,
) -> Result<Json<Command>, Status> {
	db.update_command(
		command_id,
		command_to_update.command.clone(),
		command_to_update.description.clone(),
	).await
	.map(Json)
	.map_err(|e| storage_error_status("updating command by id", &e, &["not found"], &["already exists"]))
}

#[options("/commands/addCommand")]
pub fn options_create_command() -> Status {
	ok_status()
}

#[options("/commands/deleteCommand")]
pub fn options_delete_command() -> Status {
	ok_status()
}

#[options("/commands/updateCommand")]
pub fn options_update_command() -> Status {
	ok_status()
}

pub fn routes() -> Vec<Route> {
	routes![
		get_command,
		list_commands,
		create_command,
		delete_command,
		update_command,
		update_command_by_id,
		options_create_command,
		options_delete_command,
		options_update_command,
	]
}

