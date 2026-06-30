// SPDX-License-Identifier: GPL-3.0-only

use rocket::{
	get, options, post, routes,
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
	pub id: i32,
	pub voter_steam_id: i64,
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
pub struct DeleteBindVoteByIdRequest {
	pub id: i32,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct DeleteBindVoteByUserRequest {
	pub voted_bind_id: i32,
}

#[get("/bind_votings/<bind_voting_id>")]
pub fn get_bind_vote(db: &State<JsonDatabase>, bind_voting_id: i32) -> Result<Json<BindVoting>, Status> {
	let binds = db.get_all_binds().map_err(|e| {
		storage_error_status("listing binds", &e, &[], &[])
	})?;

	let mut vote_id = 0;
	for bind in binds {
		for steam_id in &bind.upvote {
			vote_id += 1;
			if vote_id == bind_voting_id {
				return Ok(Json(BindVoting {
					id: vote_id,
					voter_steam_id: *steam_id,
					voted_bind_id: bind.id,
					vote: "Upvote".to_string(),
				}));
			}
		}
		for steam_id in &bind.downvote {
			vote_id += 1;
			if vote_id == bind_voting_id {
				return Ok(Json(BindVoting {
					id: vote_id,
					voter_steam_id: *steam_id,
					voted_bind_id: bind.id,
					vote: "Downvote".to_string(),
				}));
			}
		}
	}

	Err(Status::NotFound)
}

#[get("/bind_votings/getBindVotings")]
pub fn list_bind_votes(db: &State<JsonDatabase>) -> Result<Json<Vec<BindVoting>>, Status> {
	let binds = db.get_all_binds().map_err(|e| {
		storage_error_status("listing binds", &e, &[], &[])
	})?;

	let mut votings = Vec::new();
	let mut vote_id = 0;

	for bind in binds {
		for steam_id in bind.upvote {
			vote_id += 1;
			votings.push(BindVoting {
				id: vote_id,
				voter_steam_id: steam_id,
				voted_bind_id: bind.id,
				vote: "Upvote".to_string(),
			});
		}
		for steam_id in bind.downvote {
			vote_id += 1;
			votings.push(BindVoting {
				id: vote_id,
				voter_steam_id: steam_id,
				voted_bind_id: bind.id,
				vote: "Downvote".to_string(),
			});
		}
	}

	Ok(Json(votings))
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
		id: 0,
		voter_steam_id: user.0,
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
		get_bind_vote,
		list_bind_votes,
		create_bind_vote,
		delete_bind_vote,
		options_create_bind_vote,
		options_delete_bind_vote,
	]
}

