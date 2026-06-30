// SPDX-License-Identifier: GPL-3.0-only

#![allow(non_snake_case)]
use gamedig::{games::l4d2, protocols::valve::game::Player};
use rocket::{get, serde::{Deserialize, Serialize, json::Json}, http::Status, State};
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::RwLock;
use std::time::{Duration, Instant};
use crate::steam_bot::registry::ConfigHandle;
use crate::utils::rate_limit::SlidingWindowLimiter;

static OPEN_LIVE_SERVER_RATE_LIMITER: OnceCell<SlidingWindowLimiter<IpAddr>> = OnceCell::new();

fn open_live_server_rate_limiter() -> &'static SlidingWindowLimiter<IpAddr> {
	OPEN_LIVE_SERVER_RATE_LIMITER.get_or_init(SlidingWindowLimiter::new)
}

fn config_snapshot(handle: &State<ConfigHandle>) -> std::sync::Arc<crate::config::Config> {
	match handle.read() {
		Ok(guard) => guard.clone(),
		Err(e) => {
			eprintln!("Warning: Config lock poisoned in LiveServerInfo: {}", e);
			e.into_inner().clone()
		}
	}
}


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

// Cache successful query responses per server endpoint.
type ServerCache = HashMap<(IpAddr, u16), (L4D2ServerInfo, Instant)>;

static LAST_GOOD_RESPONSE: OnceCell<RwLock<ServerCache>> = OnceCell::new();

/// Initialize the cache
fn get_cache() -> &'static RwLock<ServerCache> {
	LAST_GOOD_RESPONSE.get_or_init(|| RwLock::new(HashMap::new()))
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
	let parsed_ip: IpAddr = ip.parse().map_err(|_| Status::BadRequest)?;
	
	// Try to query with retries
	for attempt in 1..=MAX_RETRIES {
		// Run blocking query in separate thread to avoid blocking async runtime
		let ip_clone = parsed_ip;
		let query_result = tokio::task::spawn_blocking(move || {
			l4d2::query(&ip_clone, Some(port))
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
					cache.insert((parsed_ip, port), (server_info.clone(), Instant::now()));
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
	if let Ok(cache) = get_cache().read()
		&& let Some((cached_info, cached_time)) = cache.get(&(parsed_ip, port))
	{
		let age = Instant::now().duration_since(*cached_time);
		if age.as_secs() < CACHE_TTL_SECS {
			eprintln!("Query failed, returning cached data (age: {}s)", age.as_secs());
			return Ok(cached_info.clone());
		} else {
			eprintln!("Query failed and cache is too old (age: {}s)", age.as_secs());
		}
	}
	
	// No cache available or too old
	Err(Status::ServiceUnavailable)
}

/// Query only the L4D2 server name without retries or cache.
///
/// This helper is intended for low-latency, best-effort lookups where the
/// caller provides its own timeout policy.
pub async fn query_server_name(ip: &str, port: u16) -> Result<String, Status> {
	let parsed_ip: IpAddr = ip.parse().map_err(|_| Status::BadRequest)?;

	let query_result = tokio::task::spawn_blocking(move || {
		l4d2::query(&parsed_ip, Some(port))
	}).await;

	match query_result {
		Ok(Ok(response)) => Ok(response.name),
		Ok(Err(e)) => {
			eprintln!("Name query failed: {}", e);
			Err(Status::ServiceUnavailable)
		}
		Err(e) => {
			eprintln!("Name query spawn error: {}", e);
			Err(Status::ServiceUnavailable)
		}
	}
}

#[get("/<ip>/<port>")]
pub async fn live_server_info(
	ip: String,
	port: u16,
	config: &State<ConfigHandle>,
	client_ip: crate::utils::client_ip::ClientIp,
) -> Result<Json<L4D2ServerInfo>, Status> {
	let config = config_snapshot(config);

	if !config.live_server_open_enabled {
		return Err(Status::NotFound);
	}

	let parsed_ip: IpAddr = ip.parse().map_err(|_| Status::BadRequest)?;

	if config.live_server_block_private_ips && crate::utils::ip_filter::is_blocked_query_target(parsed_ip) {
		return Err(Status::Forbidden);
	}

	let client_ip = client_ip.0;
	if !open_live_server_rate_limiter().check_and_record(
		client_ip,
		config.live_server_rate_limit_per_ip,
		config.live_server_rate_limit_burst,
	) {
		return Err(Status::TooManyRequests);
	}

	query_server_with_retry(&ip, port).await.map(Json)
}

#[get("/kether")]
pub async fn live_server_info_kether(config: &State<ConfigHandle>) -> Result<Json<L4D2ServerInfo>, Status> {
	let config = config_snapshot(config);
	query_server_with_retry(config.server_ip(), config.server_port()).await
		.map(Json)
}

#[get("/kether2")]
pub async fn live_server_info_kether2(config: &State<ConfigHandle>) -> Result<Json<L4D2ServerInfo>, Status> {
	let config = config_snapshot(config);
	query_server_with_retry(config.server2_ip(), config.server2_port()).await
		.map(Json)
}