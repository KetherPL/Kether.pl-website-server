// SPDX-License-Identifier: GPL-3.0-only

mod steam_openid;

use crate::config::Config;
use crate::steam_bot::registry::ConfigHandle;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rocket::http::{Cookie, SameSite, Status};
use rocket::request::{FromRequest, Outcome, Request};
use rocket::time::Duration;
use rocket::State;
use rocket::serde::{Deserialize, Serialize};

pub use steam_openid::mount_auth_routes;

pub const SESSION_COOKIE_NAME: &str = "session";

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Claims {
	sub: i64,
	exp: usize,
}

/// Mint a signed session JWT for the given Steam ID.
pub fn mint_session(steam_id: i64, config: &Config) -> Result<String, jsonwebtoken::errors::Error> {
	let exp = chrono::Utc::now()
		.checked_add_signed(chrono::Duration::hours(config.auth_session_ttl_hours as i64))
		.map(|dt| dt.timestamp() as usize)
		.unwrap_or(0);

	let claims = Claims { sub: steam_id, exp };
	encode(
		&Header::default(),
		&claims,
		&EncodingKey::from_secret(config.session_secret.as_bytes()),
	)
}

/// Verify a session JWT and return the Steam ID if valid.
pub fn verify_session(token: &str, secret: &str) -> Option<i64> {
	let token_data = decode::<Claims>(
		token,
		&DecodingKey::from_secret(secret.as_bytes()),
		&Validation::default(),
	)
	.ok()?;
	Some(token_data.claims.sub)
}

/// Build the HttpOnly session cookie for a freshly minted JWT.
pub fn build_session_cookie(token: &str, config: &Config) -> Cookie<'static> {
	let cross_site = config.auth_frontend_url.starts_with("https://");
	let mut cookie = Cookie::build((SESSION_COOKIE_NAME, token.to_string()))
		.path("/")
		.http_only(true)
		.secure(cross_site)
		.same_site(if cross_site {
			SameSite::None
		} else {
			SameSite::Lax
		})
		.max_age(Duration::seconds(config.auth_session_max_age_secs()));

	if let Some(domain) = config.auth_cookie_domain() {
		cookie = cookie.domain(domain.to_string());
	}

	cookie.into()
}

/// Build a cookie that clears the session.
pub fn clear_session_cookie(config: &Config) -> Cookie<'static> {
	let cross_site = config.auth_frontend_url.starts_with("https://");
	let mut cookie = Cookie::build((SESSION_COOKIE_NAME, String::new()))
		.path("/")
		.http_only(true)
		.secure(cross_site)
		.same_site(if cross_site {
			SameSite::None
		} else {
			SameSite::Lax
		})
		.max_age(Duration::ZERO);

	if let Some(domain) = config.auth_cookie_domain() {
		cookie = cookie.domain(domain.to_string());
	}

	cookie.into()
}

fn config_snapshot(handle: &State<ConfigHandle>) -> std::sync::Arc<Config> {
	match handle.read() {
		Ok(guard) => guard.clone(),
		Err(e) => {
			eprintln!("Warning: Config lock poisoned in auth: {}", e);
			e.into_inner().clone()
		}
	}
}

/// Authenticated user extracted from the session cookie.
pub struct AuthUser(pub i64);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
	type Error = ();

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		let config_handle = match request.guard::<&State<ConfigHandle>>().await {
			Outcome::Success(handle) => handle,
			Outcome::Error(e) => return Outcome::Error(e),
			Outcome::Forward(f) => return Outcome::Forward(f),
		};

		let config = config_snapshot(config_handle);
		let token = request
			.cookies()
			.get(SESSION_COOKIE_NAME)
			.map(|cookie| cookie.value().to_string());

		let Some(token) = token else {
			return Outcome::Error((Status::Unauthorized, ()));
		};

		match verify_session(&token, &config.session_secret) {
			Some(steam_id) => Outcome::Success(AuthUser(steam_id)),
			None => Outcome::Error((Status::Unauthorized, ())),
		}
	}
}

/// Admin user — must be authenticated and listed in `frontend_admins`.
pub struct AdminUser(pub i64);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AdminUser {
	type Error = ();

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		match AuthUser::from_request(request).await {
			Outcome::Success(AuthUser(steam_id)) => {
				let config_handle = match request.guard::<&State<ConfigHandle>>().await {
					Outcome::Success(handle) => handle,
					Outcome::Error(e) => return Outcome::Error(e),
					Outcome::Forward(f) => return Outcome::Forward(f),
				};

				let config = config_snapshot(config_handle);
				if config.is_admin(steam_id) {
					Outcome::Success(AdminUser(steam_id))
				} else {
					Outcome::Error((Status::Forbidden, ()))
				}
			}
			Outcome::Error(e) => Outcome::Error(e),
			Outcome::Forward(f) => Outcome::Forward(f),
		}
	}
}
