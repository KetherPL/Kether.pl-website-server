// SPDX-License-Identifier: GPL-3.0-only

use rocket::{
	options, post, routes,
	http::Status,
	serde::json::Json,
	Route, State,
};
use rocket::serde::{Deserialize, Serialize};

use crate::json_api::utils::{ok_status, storage_error_status};
use crate::json_storage::JsonDatabase;
use crate::auth::AuthUser;

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct BindVoting {
	pub voted_bind_id: i32,
	pub vote: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct CreateBindVoteRequest {
	pub voted_bind_id: i32,
	pub vote: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DeleteBindVoteByUserRequest {
	pub voted_bind_id: i32,
}

#[post("/bind_votings/addBindVoting", data = "<request>")]
pub async fn create_bind_vote(
	user: AuthUser,
	db: &State<JsonDatabase>,
	request: Json<CreateBindVoteRequest>,
) -> Result<Json<BindVoting>, Status> {
	db.add_vote(
		request.voted_bind_id,
		user.0,
		&request.vote,
	).await
	.map_err(|e| storage_error_status("adding bind vote", &e, &["not found"], &[]))?;

	Ok(Json(BindVoting {
		voted_bind_id: request.voted_bind_id,
		vote: request.vote.clone(),
	}))
}

#[post("/bind_votings/deleteBindVoting", data = "<request>")]
pub async fn delete_bind_vote(
	user: AuthUser,
	db: &State<JsonDatabase>,
	request: Json<DeleteBindVoteByUserRequest>,
) -> Result<Json<usize>, Status> {
	db.remove_vote(
		request.voted_bind_id,
		user.0,
	).await
	.map(|_| Json(1))
	.map_err(|e| storage_error_status("removing bind vote", &e, &["not found"], &[]))
}

#[options("/bind_votings/addBindVoting")]
pub fn options_create_bind_vote() -> Status {
	ok_status()
}

#[options("/bind_votings/deleteBindVoting")]
pub fn options_delete_bind_vote() -> Status {
	ok_status()
}

pub fn routes() -> Vec<Route> {
	routes![
		create_bind_vote,
		delete_bind_vote,
		options_create_bind_vote,
		options_delete_bind_vote,
	]
}
