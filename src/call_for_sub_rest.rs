// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;
use edon::{Nodejs, self};
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
pub async fn call_for_sub(payload: Json<CallForSubPayload>, config: &State<Config>, node: &State<Arc<Nodejs>>) -> Result<(), Status> {
	let steam = Steam::new(&config.steam_web_api_key);

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
    // println!("Call for sub from: {}", caller_name?);
    let acc_id = SteamId::get_account_id(&steam_id_parsed);
    // println!("Account ID: {}", acc_id);
    // TODO: need a NodeJS bridge to pass the message to the working steam-bot. The printlns are to be removed after.
    let sub_msg: String = format!("[mention={}]@{}[/mention] called for a sub, [mention=here]@online[/mention]", acc_id, caller_name?);
    println!("Message: {}", sub_msg);

    // Call Node.js `sendMessage` function
    let node_context = match node.spawn_context() {
        Ok(ctx) => ctx,
        Err(e) => {
            eprintln!("Failed to spawn Node.js context: {:?}", e);
            return Err(Status::InternalServerError);
        }
    };


	// Use eval to call the global sendMessage function
	let js_code = format!(
		r#"
		(async () => {{
			try {{
				if (typeof global.sendMessage === 'function') {{
					console.log('Calling global.sendMessage with:', '{}');
					await global.sendMessage('{}');
					console.log('Message sent successfully');
					return 'Message sent successfully';
				}} else {{
					console.error('global.sendMessage is not available');
					throw new Error('global.sendMessage is not available');
				}}
			}} catch (error) {{
				console.error('Error sending message:', error);
				throw error;
			}}
		}})();
		"#,
		sub_msg.replace("'", "\\'"), // For logging
		sub_msg.replace("'", "\\'")  // Escape single quotes in the message
	);

	let result = node_context.eval(&js_code);

	match result {
		Ok(_) => {
			println!("Message sent successfully via Steam bot");
			Ok(())
		}
		Err(e) => {
			eprintln!("Failed to send message via Node.js: {:?}", e);
			Err(Status::InternalServerError)
		}
	}
}

pub fn mount_callForSub_routes() -> Vec<rocket::Route> {
    routes![
		call_for_sub,
    ]
}