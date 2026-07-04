// SPDX-License-Identifier: GPL-3.0-only

mod csrf;
mod exchange;
mod steam_openid;

use crate::config::Config;
use crate::steam_bot::registry::ConfigHandle;
use csrf::CsrfGuard;
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use rocket::http::{Cookie, SameSite, Status};
use rocket::http::uri::Host;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::time::Duration;
use rocket::State;
use rocket::serde::{Deserialize, Serialize};

pub use exchange::ExchangeCodeStore;
pub use steam_openid::mount_auth_routes;

pub const SESSION_COOKIE_NAME: &str = "session";

#[derive(Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct Claims {
	sub: String,
	exp: usize,
	#[serde(default)]
	adm: bool,
}

/// Mint a signed session JWT for the given Steam ID.
pub fn mint_session(steam_id: i64, config: &Config) -> Result<String, jsonwebtoken::errors::Error> {
	let exp = chrono::Utc::now()
		.checked_add_signed(chrono::Duration::hours(config.auth_session_ttl_hours as i64))
		.map(|dt| dt.timestamp() as usize)
		.unwrap_or(0);

	let claims = Claims {
		sub: steam_id.to_string(),
		exp,
		adm: config.is_admin(steam_id),
	};
	encode(
		&Header::default(),
		&claims,
		&EncodingKey::from_secret(config.session_secret.as_bytes()),
	)
}

/// Extract a Bearer token from the Authorization header.
pub fn extract_bearer_token(request: &Request<'_>) -> Option<String> {
	let header = request.headers().get_one("Authorization")?;
	parse_bearer_header(header)
}

fn parse_bearer_header(header: &str) -> Option<String> {
	const PREFIX: &str = "Bearer ";
	if !header.starts_with(PREFIX) {
		return None;
	}
	let token = header[PREFIX.len()..].trim();
	if token.is_empty() {
		return None;
	}
	Some(token.to_string())
}

/// Verify a session JWT and return the Steam ID if valid.
pub fn verify_session(token: &str, secret: &str) -> Option<i64> {
	let token_data = decode::<Claims>(
		token,
		&DecodingKey::from_secret(secret.as_bytes()),
		&Validation::default(),
	)
	.ok()?;
	token_data.claims.sub.parse().ok()
}

/// Whether the session cookie is sent in a cross-site context (frontend origin ≠ API origin).
pub fn auth_cookie_is_cross_site(config: &Config, api_origin: &str) -> bool {
	fn normalize_origin(url: &str) -> String {
		url.trim()
			.trim_end_matches('/')
			.to_ascii_lowercase()
	}

	normalize_origin(&config.auth_frontend_url) != normalize_origin(api_origin)
}

/// Build the HttpOnly session cookie for a freshly minted JWT.
pub fn build_session_cookie(token: &str, config: &Config, api_origin: &str) -> Cookie<'static> {
	let cross_site = auth_cookie_is_cross_site(config, api_origin);
	let secure = api_origin.starts_with("https://") || config.auth_frontend_url.starts_with("https://");
	let mut cookie = Cookie::build((SESSION_COOKIE_NAME, token.to_string()))
		.path("/")
		.http_only(true)
		.secure(secure)
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
pub fn clear_session_cookie(config: &Config, api_origin: &str) -> Cookie<'static> {
	let cross_site = auth_cookie_is_cross_site(config, api_origin);
	let secure = api_origin.starts_with("https://") || config.auth_frontend_url.starts_with("https://");
	let mut cookie = Cookie::build((SESSION_COOKIE_NAME, String::new()))
		.path("/")
		.http_only(true)
		.secure(secure)
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

/// Derive the API origin from the incoming Host header (for cookie SameSite/Secure).
pub fn api_origin_from_host(host: &Host<'_>) -> String {
	let host_str = host.to_string();
	if host_str.starts_with("localhost") || host_str.starts_with("127.0.0.1") {
		format!("http://{host_str}")
	} else {
		format!("https://{host_str}")
	}
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

/// Authenticated user extracted from the Authorization Bearer header.
pub struct AuthUser(pub i64);

/// Optional Bearer token from the Authorization header (for public session introspection).
pub struct OptionalBearer(pub Option<String>);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for OptionalBearer {
	type Error = ();

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		Outcome::Success(OptionalBearer(extract_bearer_token(request)))
	}
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for AuthUser {
	type Error = ();

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		let config_handle = match request.guard::<&State<ConfigHandle>>().await {
			Outcome::Success(handle) => handle,
			Outcome::Error(e) => return Outcome::Error(e),
			Outcome::Forward(f) => return Outcome::Forward(f),
		};

		if let Outcome::Error(e) = CsrfGuard::from_request(request).await {
			return Outcome::Error(e);
		}

		let config = config_snapshot(config_handle);

		let Some(token) = extract_bearer_token(request) else {
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

#[cfg(test)]
mod tests {
	use super::*;

	fn test_config(frontend_url: &str, admins: &[i64]) -> Config {
		let admins_list = admins
			.iter()
			.map(|id| id.to_string())
			.collect::<Vec<_>>()
			.join(", ");
		Config::from_toml_str(&format!(
			r#"
frontend_admins = [{admins_list}]
[auth]
frontend_url = "{frontend_url}"
"#
		))
		.expect("config")
	}

	#[test]
	fn auth_cookie_same_site_when_frontend_matches_api() {
		let config = test_config("https://21370000.xyz", &[]);
		assert!(!auth_cookie_is_cross_site(
			&config,
			"https://21370000.xyz"
		));
	}

	#[test]
	fn auth_cookie_cross_site_when_frontend_differs_from_api() {
		let config = test_config("https://kether.pl", &[]);
		assert!(auth_cookie_is_cross_site(
			&config,
			"https://21370000.xyz"
		));
	}

	#[test]
	fn parse_bearer_header_accepts_valid_token() {
		assert_eq!(
			parse_bearer_header("Bearer jwt-token-123"),
			Some("jwt-token-123".to_string())
		);
	}

	#[test]
	fn parse_bearer_header_rejects_invalid() {
		for header in ["", "Basic abc", "Bearer ", "bearer x"] {
			assert_eq!(parse_bearer_header(header), None);
		}
	}

	#[test]
	fn verify_session_round_trip() {
		let config = test_config("https://kether.pl", &[]);
		let token = mint_session(76561198000000000, &config).expect("mint");
		assert_eq!(
			verify_session(&token, &config.session_secret),
			Some(76561198000000000)
		);
	}

	#[test]
	fn mint_session_includes_adm_claim() {
		let admin_id = 76561198000000000_i64;
		let user_id = 76561198000000001_i64;
		let config = test_config("https://kether.pl", &[admin_id]);

		let admin_token = mint_session(admin_id, &config).expect("mint admin");
		let user_token = mint_session(user_id, &config).expect("mint user");

		let decode_claims = |token: &str| -> Claims {
			decode::<Claims>(
				token,
				&DecodingKey::from_secret(config.session_secret.as_bytes()),
				&Validation::default(),
			)
			.expect("decode")
			.claims
		};

		assert!(decode_claims(&admin_token).adm);
		assert!(!decode_claims(&user_token).adm);
	}
}
