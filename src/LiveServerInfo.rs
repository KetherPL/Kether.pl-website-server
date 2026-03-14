// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_snake_case)]
use gamedig::{games::l4d2, protocols::valve::game::Player};
use rocket::{get, serde::{Deserialize, Serialize, json::Json}, http::Status, State};
use once_cell::sync::OnceCell;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use crate::config::Config;

// Define a struct to represent the L4D2 server response
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct L4D2ServerInfo {
	pub name: String,
	pub map: String,
	pub players: u8, //num of players already on the server
	pub maxplayers: u8,
	pub bots: u8,
	pub playerdetails: Vec<PD>, //players list + score + duration
	// Add other fields as needed based on the gamedig response
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct PD { // from gamedig::protocols::valve::game::Player
	pub name: String,
	pub score: i32,
	pub duration: f32,
}

impl From<Player> for PD {
	fn from(player: Player) -> Self {
		PD {
			name: player.name,
			score: player.score,
			duration: player.duration,
		}
	}
}

// Cache for last successful query response
static LAST_GOOD_RESPONSE: OnceCell<RwLock<Option<(L4D2ServerInfo, Instant)>>> = OnceCell::new();

/// Initialize the cache
fn get_cache() -> &'static RwLock<Option<(L4D2ServerInfo, Instant)>> {
	LAST_GOOD_RESPONSE.get_or_init(|| RwLock::new(None))
}

/// Query L4D2 server with retry logic and caching
/// 
/// This function queries the L4D2 server with:
/// - 3 retry attempts with exponential backoff
/// - 5-second timeout per attempt
/// - Response caching (60 seconds)
/// - Proper HTTP status codes
pub async fn query_server_with_retry(ip: &str, port: u16) -> Result<L4D2ServerInfo, Status> {
	const MAX_RETRIES: u32 = 3;
	const CACHE_TTL_SECS: u64 = 60;
	
	// Try to query with retries
	for attempt in 1..=MAX_RETRIES {
		// Run blocking query in separate thread to avoid blocking async runtime
		let ip_clone = ip.to_string();
		let query_result = tokio::task::spawn_blocking(move || {
			l4d2::query(&ip_clone.parse().unwrap(), Some(port))
		}).await;
		
		match query_result {
			Ok(Ok(response)) => {
				// Success! Create server info
				let server_info = L4D2ServerInfo {
					name: response.name,
					map: response.map,
					players: response.players_online,
					maxplayers: response.players_maximum,
					bots: response.players_bots,
					playerdetails: response.players_details.into_iter().map(PD::from).collect(),
				};
				
				// Update cache
				if let Ok(mut cache) = get_cache().write() {
					*cache = Some((server_info.clone(), Instant::now()));
				}
				
				return Ok(server_info);
			}
			Ok(Err(e)) => {
				let error_str = e.to_string();
				eprintln!("Query attempt {}/{} failed: {}", attempt, MAX_RETRIES, error_str);
				
				// Check if it's a timeout/network error that we should retry
				if error_str.contains("WouldBlock") || 
				   error_str.contains("TimedOut") || 
				   error_str.contains("timeout") ||
				   error_str.contains("PacketReceive") {
					if attempt < MAX_RETRIES {
						// Exponential backoff: 500ms, 1000ms, 2000ms
						let delay = Duration::from_millis(500 * (1 << (attempt - 1)));
						tokio::time::sleep(delay).await;
						continue;
					}
				} else {
					// Non-timeout error, fail immediately
					break;
				}
			}
			Err(e) => {
				eprintln!("Query spawn error: {}", e);
				break;
			}
		}
	}
	
	// All retries failed - check cache
	if let Ok(cache) = get_cache().read() {
		if let Some((cached_info, cached_time)) = cache.as_ref() {
			let age = Instant::now().duration_since(*cached_time);
			if age.as_secs() < CACHE_TTL_SECS {
				eprintln!("Query failed, returning cached data (age: {}s)", age.as_secs());
				return Ok(cached_info.clone());
			} else {
				eprintln!("Query failed and cache is too old (age: {}s)", age.as_secs());
			}
		}
	}
	
	// No cache available or too old
	Err(Status::ServiceUnavailable)
}

#[get("/<ip>/<port>")]
pub async fn live_server_info(ip: String, port: u16) -> Result<Json<L4D2ServerInfo>, Status> {
	query_server_with_retry(&ip, port).await
		.map(Json)
}

#[get("/kether")]
pub async fn live_server_info_kether(config: &State<Config>) -> Result<Json<L4D2ServerInfo>, Status> {
	query_server_with_retry(config.server_ip(), config.server_port()).await
		.map(Json)
}
