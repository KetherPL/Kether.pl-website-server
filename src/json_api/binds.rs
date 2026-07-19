// SPDX-License-Identifier: GPL-3.0-only

use rocket::{
	get, options, post, put, routes,
	http::Status,
	serde::json::Json,
	Route, State,
};
use rocket::serde::{Deserialize, Serialize};

use crate::json_api::models::{bind_to_public, Bind, BindPublic};
use crate::json_api::utils::{ok_status, storage_error_status};
use crate::json_storage::JsonDatabase;
use crate::auth::{AdminUser, AuthUser};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct CreateBindRequest {
	pub author: String,
	pub text: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DeleteBindRequest {
	pub id: i32,
}

#[get("/binds/<bind_id>")]
pub fn get_bind(
	db: &State<JsonDatabase>,
	bind_id: i32,
	viewer: Option<AuthUser>,
) -> Result<Json<BindPublic>, Status> {
	let viewer_id = viewer.map(|user| user.0);
	db.get_bind(bind_id)
		.map(|bind| Json(bind_to_public(&bind, viewer_id)))
		.map_err(|e| storage_error_status("getting bind", &e, &["not found"], &[]))
}

#[get("/binds/getBinds")]
pub fn list_binds(
	db: &State<JsonDatabase>,
	viewer: Option<AuthUser>,
) -> Result<Json<Vec<BindPublic>>, Status> {
	let viewer_id = viewer.map(|user| user.0);
	db.get_all_binds()
		.map(|binds| {
			Json(
				binds
					.iter()
					.map(|bind| bind_to_public(bind, viewer_id))
					.collect(),
			)
		})
		.map_err(|e| storage_error_status("listing binds", &e, &[], &[]))
}

#[post("/binds/addBind", data = "<request>")]
pub async fn create_bind(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	request: Json<CreateBindRequest>,
) -> Result<Json<Bind>, Status> {
	db.create_bind(request.author.clone(), request.text.clone()).await
		.map(Json)
		.map_err(|e| storage_error_status("creating bind", &e, &[], &["already exists"]))
}

#[post("/binds/deleteBind", data = "<request>")]
pub async fn delete_bind(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	request: Json<DeleteBindRequest>,
) -> Result<Json<usize>, Status> {
	db.delete_bind(request.id).await
		.map(|_| Json(1))
		.map_err(|e| storage_error_status("deleting bind", &e, &["not found"], &[]))
}

#[post("/binds/updateBind", data = "<bind_to_update>")]
pub async fn update_bind(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	bind_to_update: Json<Bind>,
) -> Result<Json<Bind>, Status> {
	db.update_bind(
		bind_to_update.id,
		bind_to_update.author.clone(),
		bind_to_update.text.clone(),
		bind_to_update.upvote.clone(),
		bind_to_update.downvote.clone(),
	).await
	.map(Json)
	.map_err(|e| storage_error_status("updating bind", &e, &["not found"], &["already exists"]))
}

#[put("/binds/<bind_id>", data = "<bind_to_update>")]
pub async fn update_bind_by_id(
	_admin: AdminUser,
	db: &State<JsonDatabase>,
	bind_id: i32,
	bind_to_update: Json<Bind>,
) -> Result<Json<Bind>, Status> {
	db.update_bind(
		bind_id,
		bind_to_update.author.clone(),
		bind_to_update.text.clone(),
		bind_to_update.upvote.clone(),
		bind_to_update.downvote.clone(),
	).await
	.map(Json)
	.map_err(|e| storage_error_status("updating bind by id", &e, &["not found"], &["already exists"]))
}

#[options("/binds/getBinds")]
pub fn options_list_binds() -> Status {
	ok_status()
}

#[options("/binds/addBind")]
pub fn options_create_bind() -> Status {
	ok_status()
}

#[options("/binds/deleteBind")]
pub fn options_delete_bind() -> Status {
	ok_status()
}

#[options("/binds/updateBind")]
pub fn options_update_bind() -> Status {
	ok_status()
}

pub fn routes() -> Vec<Route> {
	routes![
		get_bind,
		list_binds,
		create_bind,
		delete_bind,
		update_bind,
		update_bind_by_id,
		options_list_binds,
		options_create_bind,
		options_delete_bind,
		options_update_bind,
	]
}

