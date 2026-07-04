// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;

use crate::auth::exchange::ExchangeCodeStore;
use crate::auth::{
	mint_session, verify_session, OptionalBearer,
};
use crate::auth::csrf::CsrfGuard;
use crate::json_api::utils::ok_status;
use crate::steam_bot::registry::ConfigHandle;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::response::Redirect;
use rocket::{get, options, post, routes, Route, State};
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};

const STEAM_OPENID_ENDPOINT: &str = "https://steamcommunity.com/openid/login";
const STEAM_CLAIMED_ID_PREFIX: &str = "https://steamcommunity.com/openid/id/";

fn config_snapshot(handle: &State<ConfigHandle>) -> std::sync::Arc<crate::config::Config> {
	match handle.read() {
		Ok(guard) => guard.clone(),
		Err(e) => {
			eprintln!("Warning: Config lock poisoned in steam_openid: {}", e);
			e.into_inner().clone()
		}
	}
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(crate = "rocket::serde")]
pub struct MeResponse {
	pub steamid: String,
	pub is_admin: bool,
}

#[derive(Deserialize)]
#[serde(crate = "rocket::serde")]
pub(crate) struct ExchangeRequest {
	code: String,
}

#[derive(Serialize)]
#[serde(crate = "rocket::serde")]
pub(crate) struct ExchangeResponse {
	access_token: String,
	steamid: String,
	is_admin: bool,
	expires_in: u64,
}

/// Parsed Steam OpenID callback query parameters.
pub struct OpenIdParams(pub HashMap<String, String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OpenIdParams {
	type Error = ();

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		let Some(query) = request.uri().query() else {
			return Outcome::Error((Status::BadRequest, ()));
		};

		let mut params = HashMap::new();
		for (key, value) in query.segments() {
			if !key.starts_with("openid.") {
				continue;
			}
			let key = urlencoding::decode(key)
				.map(|cow| cow.into_owned())
				.unwrap_or_else(|_| key.to_string());
			let value = urlencoding::decode(value)
				.map(|cow| cow.into_owned())
				.unwrap_or_else(|_| value.to_string());
			params.insert(key, value);
		}

		if params.is_empty() {
			return Outcome::Error((Status::BadRequest, ()));
		}

		Outcome::Success(OpenIdParams(params))
	}
}

fn parse_steam_id_from_claimed_id(claimed_id: &str) -> Option<i64> {
	let steam_id_str = claimed_id.strip_prefix(STEAM_CLAIMED_ID_PREFIX)?;
	if steam_id_str.len() != 17 || !steam_id_str.chars().all(|c| c.is_ascii_digit()) {
		return None;
	}
	steam_id_str.parse().ok()
}

async fn verify_openid_response(params: &HashMap<String, String>) -> Result<(), Status> {
	if params.get("openid.mode").map(String::as_str) != Some("id_res") {
		eprintln!("Steam OpenID callback: expected openid.mode=id_res");
		return Err(Status::BadRequest);
	}

	let mut verification_params = params.clone();
	verification_params.insert("openid.mode".to_string(), "check_authentication".to_string());

	let client = reqwest::Client::new();
	let response = client
		.post(STEAM_OPENID_ENDPOINT)
		.form(&verification_params)
		.send()
		.await
		.map_err(|e| {
			eprintln!("Steam OpenID verification request failed: {}", e);
			Status::InternalServerError
		})?;

	let body = response.text().await.map_err(|e| {
		eprintln!("Steam OpenID verification response read failed: {}", e);
		Status::InternalServerError
	})?;

	let mut is_valid = false;
	for line in body.lines() {
		let Some((key, value)) = line.split_once(':') else {
			continue;
		};
		if key.trim() == "is_valid" && value.trim() == "true" {
			is_valid = true;
			break;
		}
	}

	if !is_valid {
		eprintln!("Steam OpenID verification failed: {}", body);
		return Err(Status::Unauthorized);
	}

	Ok(())
}

#[get("/steam/callback")]
pub async fn steam_callback(
	openid: OpenIdParams,
	config: &State<ConfigHandle>,
	exchange_store: &State<ExchangeCodeStore>,
) -> Result<Redirect, Status> {
	let params = &openid.0;

	verify_openid_response(params).await?;

	let claimed_id = params
		.get("openid.claimed_id")
		.ok_or(Status::BadRequest)?;

	let steam_id = parse_steam_id_from_claimed_id(claimed_id).ok_or_else(|| {
		eprintln!("Steam OpenID callback: invalid claimed_id {}", claimed_id);
		Status::BadRequest
	})?;

	let config = config_snapshot(config);
	let code = exchange_store.issue_code(steam_id);
	let frontend_base = config.auth_frontend_url.trim_end_matches('/');
	let callback_url = format!("{frontend_base}/auth/callback?code={code}");
	Ok(Redirect::to(callback_url))
}

#[post("/exchange", data = "<body>")]
pub fn auth_exchange(
	body: Json<ExchangeRequest>,
	config: &State<ConfigHandle>,
	exchange_store: &State<ExchangeCodeStore>,
	_csrf: CsrfGuard,
) -> Result<Json<ExchangeResponse>, Status> {
	let config = config_snapshot(config);

	let code = body.code.trim();
	if code.is_empty() {
		return Err(Status::BadRequest);
	}

	let Some(steam_id) = exchange_store.consume_code(&code) else {
		eprintln!("Auth exchange: invalid or expired code");
		return Err(Status::BadRequest);
	};

	let access_token = mint_session(steam_id, &config).map_err(|e| {
		eprintln!("Failed to mint session JWT: {}", e);
		Status::InternalServerError
	})?;

	let expires_in = config.auth_session_max_age_secs().max(0) as u64;

	Ok(Json(ExchangeResponse {
		access_token,
		steamid: steam_id.to_string(),
		is_admin: config.is_admin(steam_id),
		expires_in,
	}))
}

#[get("/me")]
pub fn auth_me(bearer: OptionalBearer, config: &State<ConfigHandle>) -> Json<Option<MeResponse>> {
	let config = config_snapshot(config);
	let response = bearer
		.0
		.and_then(|token| verify_session(&token, &config.session_secret))
		.map(|steam_id| MeResponse {
			steamid: steam_id.to_string(),
			is_admin: config.is_admin(steam_id),
		});
	Json(response)
}

#[post("/logout")]
pub fn auth_logout(_csrf: CsrfGuard) -> Status {
	ok_status()
}

#[options("/steam/callback")]
pub fn options_steam_callback() -> Status {
	ok_status()
}

#[options("/exchange")]
pub fn options_auth_exchange() -> Status {
	ok_status()
}

#[options("/me")]
pub fn options_auth_me() -> Status {
	ok_status()
}

#[options("/logout")]
pub fn options_auth_logout() -> Status {
	ok_status()
}

pub fn mount_auth_routes() -> Vec<Route> {
	routes![
		steam_callback,
		auth_exchange,
		auth_me,
		auth_logout,
		options_steam_callback,
		options_auth_exchange,
		options_auth_me,
		options_auth_logout,
	]
}
