// SPDX-License-Identifier: GPL-3.0-only

use rocket::{http::Status, post, routes, serde::json::Json, State};
use steam_rs::{steam_id::SteamId, Steam};
use rocket::serde::Deserialize;
use crate::config::Config;
use colored::Colorize;

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CallForSubPayload {
	#[serde(rename = "steamID")]
	steam_id: u64,
}

// --- Call For Sub ---

#[post("/", data = "<payload>")]
pub async fn call_for_sub(payload: Json<CallForSubPayload>, config: &State<Config>) -> Result<(), Status> {
	let steam = Steam::new(&config.steam_web_api_key);

	// Parse the steam_id string to u64
	// let steam_id_u64: u64 = match payload.steam_id.parse() {
	// 	Ok(id) => id,
	// 	Err(e) => {
	// 		eprintln!("Invalid SteamID64 format: {}", e);
	// 		return Err(Status::BadRequest);
	// 	}
	// };

	// Create a SteamId from the u64
	let steam_id_parsed = SteamId::new(payload.steam_id);
	let steam_ids = vec![steam_id_parsed];

	// Get player summaries
	let caller_name: Result<String, Status> = match steam.get_player_summaries(steam_ids).await {
		Ok(response) => {
			if let Some(player) = response.first() {
				Ok(player.persona_name.clone())
			} else {
			    eprintln!("{} Invalid SteamID64 format: {}", "Error:".red(), steam_id_parsed);
				Err(Status::BadRequest)
			}
		}
		Err(e) => {
			eprintln!("{} Could not fetch caller name for SteamID: {} — Error: {}", "Error:".red(), steam_id_parsed, e);
			Err(Status::InternalServerError)
		}
	};
    println!("Call for sub from: {}", caller_name.clone()?);
    let acc_id = SteamId::get_account_id(&steam_id_parsed);
    println!("Account ID: {}", acc_id);
	let sub_msg: String = format!("[mention={}]@{}[/mention] called for a sub, [mention=here]@online[/mention]", acc_id, caller_name?);
    println!("Message: {}", sub_msg);
	
	//... TODO
    Ok(())
}

pub fn mount_callForSub_routes() -> Vec<rocket::Route> {
    routes![
		call_for_sub,
    ]
}