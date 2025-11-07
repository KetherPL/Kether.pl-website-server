// SPDX-License-Identifier: GPL-3.0-only

use rocket::serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Bind {
	pub id: i32,
	pub author: String,
	pub text: String,
	pub upvote: Vec<i64>,
	pub downvote: Vec<i64>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct BindSuggestion {
	pub id: i32,
	pub author: String,
	pub text: String,
	pub proposed_by: String,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct Command {
	pub id: i32,
	pub command: String,
	pub description: String,
}


