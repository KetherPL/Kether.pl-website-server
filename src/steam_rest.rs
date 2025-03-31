// SPDX-License-Identifier: GPL-3.0-only

use rocket::{post, routes, serde::json::Json, http::Status, State};
use steam_rs::{steam_user::get_player_summaries::Player, steam_id::SteamId, Steam};
use rocket::serde::{Serialize, Deserialize};
use crate::config::Config;

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub struct SteamUserDetails {
	pub personaname: String,
	pub profileurl: String,
	pub avatar: String,
	pub avatarmedium: String,
	pub avatarfull: String,
	pub realname: Option<String>,
	pub loccountrycode: Option<String>,
	pub steamid: String,
}

impl From<Player> for SteamUserDetails {
	fn from(player: Player) -> Self {
		SteamUserDetails {
			personaname: player.persona_name,
			profileurl: player.profile_url,
			avatar: player.avatar,
			avatarmedium: player.avatar_medium,
			avatarfull: player.avatar_full,
			realname: player.real_name,
			loccountrycode: player.loc_country_code,
			steamid: player.steam_id.to_string(),
		}
	}
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub struct GamesInfo {
	pub owns_left4dead2: bool,
}

#[post("/userData", data = "<steam_id>")]
pub async fn get_user_data(steam_id: String, config: &State<Config>) -> Result<Json<SteamUserDetails>, Status> {
	let steam = Steam::new(&config.steam_web_api_key);

	// Parse the steam_id string to u64
	let steam_id_u64: u64 = match steam_id.parse() {
		Ok(id) => id,
		Err(e) => {
			eprintln!("Invalid Steam ID format: {}", e);
			return Err(Status::BadRequest);
		}
	};

	// Create a SteamId from the u64
	let steam_id_parsed = SteamId::new(steam_id_u64);
	let steam_ids = vec![steam_id_parsed];

	// Get player summaries
	match steam.get_player_summaries(steam_ids).await {
		Ok(response) => {
			if let Some(player) = response.first() {
				let steam_user_details = SteamUserDetails::from(player.clone());
				Ok(Json(steam_user_details))
			} else {
				Err(Status::NotFound)
			}
		}
		Err(e) => {
			eprintln!("Error fetching user data: {}", e);
			Err(Status::InternalServerError)
		}
	}
}

#[post("/games", data = "<steam_id>")] //Just check if the user has L4D2 bought on his account
pub async fn get_user_games(steam_id: String, config: &State<Config>) -> Result<Json<GamesInfo>, Status> {
	let steam = Steam::new(&config.steam_web_api_key);

	// Parse the steam_id string to u64
	let steam_id_u64: u64 = match steam_id.parse() {
		Ok(id) => id,
		Err(e) => {
			eprintln!("Invalid Steam ID format: {}", e);
			return Err(Status::BadRequest);
		}
	};

	// Create a SteamId from the u64
	let steam_id_parsed = SteamId::new(steam_id_u64);

	match steam.get_owned_games(steam_id_parsed, true, false, 550, false, None, "en", false).await {
		Ok(response) => {
			let mut owns_left4dead2 = false;
			for game in response.games {
				if game.appid == 550 { // Left 4 Dead 2 app ID
					owns_left4dead2 = true;
					break;
				}
			}
			let games_info = GamesInfo {
				owns_left4dead2,
			};
			Ok(Json(games_info))
		}
		Err(e) => {
			eprintln!("Error fetching user games: {}", e);
			Err(Status::InternalServerError)
		}
	}
}

pub fn mount_steam_routes() -> Vec<rocket::Route> {
	routes![get_user_data, get_user_games]
}
