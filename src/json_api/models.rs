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

/// Public bind view returned from GET endpoints (no voter Steam IDs).
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
#[serde(crate = "rocket::serde")]
pub struct BindPublic {
	pub id: i32,
	pub author: String,
	pub text: String,
	pub upvote_count: u32,
	pub downvote_count: u32,
	#[serde(skip_serializing_if = "Option::is_none")]
	pub self_vote: Option<String>,
}

pub fn bind_to_public(bind: &Bind, viewer_steam_id: Option<i64>) -> BindPublic {
	let self_vote = viewer_steam_id.and_then(|viewer| {
		if bind.upvote.contains(&viewer) {
			Some("Upvote".to_string())
		} else if bind.downvote.contains(&viewer) {
			Some("Downvote".to_string())
		} else {
			None
		}
	});

	BindPublic {
		id: bind.id,
		author: bind.author.clone(),
		text: bind.text.clone(),
		upvote_count: bind.upvote.len() as u32,
		downvote_count: bind.downvote.len() as u32,
		self_vote,
	}
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

#[cfg(test)]
mod tests {
	use super::*;

	fn sample_bind() -> Bind {
		Bind {
			id: 1,
			author: "Player".to_string(),
			text: "bind text".to_string(),
			upvote: vec![111, 222],
			downvote: vec![333],
		}
	}

	#[test]
	fn bind_to_public_counts_votes() {
		let public = bind_to_public(&sample_bind(), None);
		assert_eq!(public.upvote_count, 2);
		assert_eq!(public.downvote_count, 1);
		assert!(public.self_vote.is_none());
	}

	#[test]
	fn bind_to_public_sets_self_vote_for_viewer() {
		let bind = sample_bind();
		assert_eq!(
			bind_to_public(&bind, Some(111)).self_vote,
			Some("Upvote".to_string())
		);
		assert_eq!(
			bind_to_public(&bind, Some(333)).self_vote,
			Some("Downvote".to_string())
		);
		assert!(bind_to_public(&bind, Some(999)).self_vote.is_none());
	}

	#[test]
	fn bind_to_public_has_no_voter_id_fields() {
		let public = bind_to_public(&sample_bind(), Some(111));
		assert_eq!(public.id, 1);
		assert_eq!(public.author, "Player");
		assert_eq!(public.text, "bind text");
	}
}

