// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::RwLock;
use std::time::{Duration, Instant};

use once_cell::sync::OnceCell;
use rocket::{http::Status, options, post, routes, serde::json::Json, State};
use steam_rs::{steam_user::get_player_summaries::Player, steam_id::SteamId, Steam};
use rocket::serde::{Serialize, Deserialize};
use crate::auth::AuthUser;
use crate::steam_bot::registry::ConfigHandle;
use crate::utils::client_ip::ClientIp;
use crate::utils::rate_limit::SlidingWindowLimiter;

static PERSONA_NAME_CACHE: OnceCell<RwLock<HashMap<u64, (String, Instant)>>> = OnceCell::new();
static USER_DATA_CACHE: OnceCell<RwLock<HashMap<u64, (Instant, SteamUserDetails)>>> = OnceCell::new();
static GAMES_CACHE: OnceCell<RwLock<HashMap<u64, (Instant, GamesInfo)>>> = OnceCell::new();
static STEAM_USER_RATE_LIMITER: OnceCell<SlidingWindowLimiter<u64>> = OnceCell::new();
static STEAM_IP_RATE_LIMITER: OnceCell<SlidingWindowLimiter<IpAddr>> = OnceCell::new();

const PERSONA_CACHE_TTL: Duration = Duration::from_secs(1800); // 30 minutes

fn persona_name_cache() -> &'static RwLock<HashMap<u64, (String, Instant)>> {
	PERSONA_NAME_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn user_data_cache() -> &'static RwLock<HashMap<u64, (Instant, SteamUserDetails)>> {
	USER_DATA_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn games_cache() -> &'static RwLock<HashMap<u64, (Instant, GamesInfo)>> {
	GAMES_CACHE.get_or_init(|| RwLock::new(HashMap::new()))
}

fn steam_user_rate_limiter() -> &'static SlidingWindowLimiter<u64> {
	STEAM_USER_RATE_LIMITER.get_or_init(SlidingWindowLimiter::new)
}

fn steam_ip_rate_limiter() -> &'static SlidingWindowLimiter<IpAddr> {
	STEAM_IP_RATE_LIMITER.get_or_init(SlidingWindowLimiter::new)
}

/// Resolve Steam IDs to persona names, using a short-lived in-memory cache.
///
/// Returns only successfully resolved IDs. Missing entries mean the caller should
/// fall back to displaying the raw Steam ID.
pub async fn resolve_persona_names(
	config: &crate::config::Config,
	ids: &[u64],
) -> HashMap<u64, String> {
	if ids.is_empty() || config.steam_web_api_key.is_empty() {
		return HashMap::new();
	}

	let now = Instant::now();
	let mut resolved = HashMap::new();
	let mut misses = Vec::new();

	if let Ok(cache) = persona_name_cache().read() {
		for &id in ids {
			if let Some((name, cached_at)) = cache.get(&id)
				&& now.duration_since(*cached_at) < PERSONA_CACHE_TTL
			{
				resolved.insert(id, name.clone());
			} else {
				misses.push(id);
			}
		}
	} else {
		misses.extend_from_slice(ids);
	}

	if misses.is_empty() {
		return resolved;
	}

	let steam = Steam::new(&config.steam_web_api_key);
	for chunk in misses.chunks(100) {
		let steam_ids: Vec<SteamId> = chunk.iter().copied().map(SteamId::new).collect();
		match steam.get_player_summaries(steam_ids).await {
			Ok(players) => {
				for player in players {
					let Ok(id) = player.steam_id.to_string().parse::<u64>() else {
						continue;
					};
					let name = player.persona_name.clone();
					resolved.insert(id, name.clone());
					if let Ok(mut cache) = persona_name_cache().write() {
						cache.insert(id, (name, now));
					}
				}
			}
			Err(e) => {
				eprintln!("Error resolving persona names: {}", e);
			}
		}
	}

	resolved
}

fn config_snapshot(handle: &State<ConfigHandle>) -> std::sync::Arc<crate::config::Config> {
	match handle.read() {
		Ok(guard) => guard.clone(),
		Err(e) => {
			eprintln!("Warning: Config lock poisoned in steam_rest: {}", e);
			e.into_inner().clone()
		}
	}
}

fn parse_and_verify_steam_id(steam_id: &str, auth: &AuthUser) -> Result<u64, Status> {
	let steam_id_u64: u64 = steam_id.parse().map_err(|e| {
		eprintln!("Invalid Steam ID format: {}", e);
		Status::BadRequest
	})?;

	if auth.0 != steam_id_u64 as i64 {
		return Err(Status::Forbidden);
	}

	Ok(steam_id_u64)
}

fn check_steam_rest_rate_limits(
	config: &crate::config::Config,
	steam_id: u64,
	client_ip: IpAddr,
) -> Result<(), Status> {
	if !steam_user_rate_limiter().check_and_record(
		steam_id,
		config.steam_rest_user_rate_limit,
		0,
	) {
		return Err(Status::TooManyRequests);
	}

	if !steam_ip_rate_limiter().check_and_record(
		client_ip,
		config.steam_rest_ip_rate_limit,
		0,
	) {
		return Err(Status::TooManyRequests);
	}

	Ok(())
}

fn response_cache_ttl(config: &crate::config::Config) -> Duration {
	Duration::from_secs(config.steam_rest_cache_ttl_secs.max(1))
}

#[derive(Serialize, Deserialize, Debug, Clone)]
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

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct GamesInfo {
	pub owns_left4dead2: bool,
}

// --- User Data ---

#[post("/userData", data = "<steam_id>")]
pub async fn get_user_data(
	steam_id: String,
	auth: AuthUser,
	config: &State<ConfigHandle>,
	client_ip: ClientIp,
) -> Result<Json<SteamUserDetails>, Status> {
	let config = config_snapshot(config);
	let steam_id_u64 = parse_and_verify_steam_id(&steam_id, &auth)?;
	check_steam_rest_rate_limits(&config, steam_id_u64, client_ip.0)?;

	let cache_ttl = response_cache_ttl(&config);
	let now = Instant::now();
	if let Ok(cache) = user_data_cache().read()
		&& let Some((cached_at, details)) = cache.get(&steam_id_u64)
		&& now.duration_since(*cached_at) < cache_ttl
	{
		return Ok(Json(details.clone()));
	}

	let steam = Steam::new(&config.steam_web_api_key);
	let steam_id_parsed = SteamId::new(steam_id_u64);
	let steam_ids = vec![steam_id_parsed];

	match steam.get_player_summaries(steam_ids).await {
		Ok(response) => {
			if let Some(player) = response.first() {
				let steam_user_details = SteamUserDetails::from(player.clone());
				if let Ok(mut cache) = user_data_cache().write() {
					cache.insert(steam_id_u64, (now, steam_user_details.clone()));
				}
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

// --- User's L4D2 posession state ---

#[post("/games", data = "<steam_id>")]
pub async fn get_user_games(
	steam_id: String,
	auth: AuthUser,
	config: &State<ConfigHandle>,
	client_ip: ClientIp,
) -> Result<Json<GamesInfo>, Status> {
	let config = config_snapshot(config);
	let steam_id_u64 = parse_and_verify_steam_id(&steam_id, &auth)?;
	check_steam_rest_rate_limits(&config, steam_id_u64, client_ip.0)?;

	let cache_ttl = response_cache_ttl(&config);
	let now = Instant::now();
	if let Ok(cache) = games_cache().read()
		&& let Some((cached_at, games_info)) = cache.get(&steam_id_u64)
		&& now.duration_since(*cached_at) < cache_ttl
	{
		return Ok(Json(games_info.clone()));
	}

	let steam = Steam::new(&config.steam_web_api_key);
	let steam_id_parsed = SteamId::new(steam_id_u64);

	match steam.get_owned_games(steam_id_parsed, true, false, 550, false, None, "en", false).await {
		Ok(response) => {
			let mut owns_left4dead2 = false;
			for game in response.games {
				if game.appid == 550 {
					owns_left4dead2 = true;
					break;
				}
			}
			let games_info = GamesInfo { owns_left4dead2 };
			if let Ok(mut cache) = games_cache().write() {
				cache.insert(steam_id_u64, (now, games_info.clone()));
			}
			Ok(Json(games_info))
		}
		Err(e) => {
			eprintln!("Error fetching user games: {}", e);
			Err(Status::InternalServerError)
		}
	}
}

// -- OPTIONS ---

#[options("/userData")]
pub async fn options_user_data() -> Status {
	Status::Ok
}

#[options("/games")]
pub async fn options_games() -> Status {
	Status::Ok
}

pub fn mount_steam_routes() -> Vec<rocket::Route> {
	routes![
		get_user_data,
		get_user_games,
		options_user_data,
		options_games,
	]
}
