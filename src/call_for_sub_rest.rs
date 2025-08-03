// SPDX-License-Identifier: GPL-3.0-only

use rocket::{http::Status, post, routes, serde::json::Json, State};
use steam_rs::{steam_id::SteamId, Steam};
use rocket::serde::Deserialize;
use crate::{config::Config, SteamBot::SteamBot};
use colored::Colorize;

/// Payload structure for call-for-sub requests
/// 
/// This struct represents the JSON payload sent to the call-for-sub endpoint.
/// It contains the Steam ID of the player requesting a substitute.
/// 
/// # Fields
/// * `steam_id` - The Steam ID (64-bit) of the player requesting a sub
/// 
/// # Example JSON
/// ```json
/// {
///   "steamID": 76561198012345678
/// }
/// ```
#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct CallForSubPayload {
	/// Steam ID of the player requesting a substitute
	/// 
	/// This field is renamed from `steam_id` to `steamID` in the JSON
	/// to match the expected API format.
	#[serde(rename = "steamID")]
	steam_id: u64,
}

// --- Call For Sub ---

/// Handles call-for-sub requests from L4D2 servers
/// 
/// This endpoint processes requests for substitutes from Left 4 Dead 2 servers.
/// It fetches the player's Steam profile information and sends a formatted
/// message to the configured Steam group chat.
/// 
/// # Process
/// 1. Receives a POST request with the player's Steam ID
/// 2. Fetches the player's Steam profile using the Steam Web API
/// 3. Formats a message with mentions for the player and online members
/// 4. Sends the message to the configured Steam group chat via SteamBot
/// 
/// # Endpoint
/// `POST /api/callForSub/`
/// 
/// # Request Body
/// ```json
/// {
///   "steamID": 76561198012345678
/// }
/// ```
/// 
/// # Response
/// * `200 OK` - Message sent successfully
/// * `400 Bad Request` - Invalid Steam ID format
/// * `500 Internal Server Error` - Steam API error or message sending failure
/// 
/// # Example
/// ```bash
/// curl -X POST http://localhost:3001/api/callForSub/ \
///   -H "Content-Type: application/json" \
///   -d '{"steamID": 76561198012345678}'
/// ```
/// 
/// # Message Format
/// The sent message follows this format:
/// `[mention=ACCOUNT_ID]@PLAYER_NAME[/mention] called for a sub, [mention=here]@online[/mention]`
/// 
/// Where:
/// * `ACCOUNT_ID` - The Steam account ID (32-bit)
/// * `PLAYER_NAME` - The player's Steam display name
/// * `@online` - Mentions all online members in the group
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
    // println!("Call for sub from: {}", caller_name.clone()?);
    let acc_id = SteamId::get_account_id(&steam_id_parsed);
    // println!("Account ID: {}", acc_id);
	let sub_msg: String = format!("[mention={}]@{}[/mention] called for a sub, [mention=here]@online[/mention]", acc_id, caller_name?);
    // println!("Message: {}", sub_msg);
	if let Err(e) = SteamBot::send_message_global(&sub_msg).await {
		eprintln!("{} Could not send message: {}", "Error:".red(), e);
	}
    Ok(())
}

/// Mounts call-for-sub REST routes
/// 
/// This function returns a vector of Rocket routes for the call-for-sub
/// functionality. It's used by the main Rocket application to mount
/// the call-for-sub endpoints.
/// 
/// # Routes
/// * `POST /` - Handles call-for-sub requests
/// 
/// # Returns
/// A vector of Rocket routes for call-for-sub functionality
/// 
/// # Example
/// ```rust
/// let routes = mount_callForSub_routes();
/// rocket_build = rocket_build.mount("/api/callForSub", routes);
/// ```
pub fn mount_callForSub_routes() -> Vec<rocket::Route> {
    routes![
		call_for_sub,
    ]
}