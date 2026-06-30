// SPDX-License-Identifier: GPL-3.0-only

use rocket::http::{Method, Status};
use rocket::request::{FromRequest, Outcome, Request};

use crate::config::Config;
use crate::steam_bot::registry::ConfigHandle;
use rocket::State;

/// Validates Origin/Referer for mutating requests.
pub struct CsrfGuard;

fn config_snapshot(handle: &State<ConfigHandle>) -> std::sync::Arc<Config> {
	match handle.read() {
		Ok(guard) => guard.clone(),
		Err(e) => {
			eprintln!("Warning: Config lock poisoned in csrf: {}", e);
			e.into_inner().clone()
		}
	}
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for CsrfGuard {
	type Error = ();

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		let config_handle = match request.guard::<&State<ConfigHandle>>().await {
			Outcome::Success(handle) => handle,
			Outcome::Error(e) => return Outcome::Error(e),
			Outcome::Forward(f) => return Outcome::Forward(f),
		};

		let config = config_snapshot(config_handle);
		match validate_csrf_origin(request, config.auth_csrf_allowed_origins()) {
			Ok(()) => Outcome::Success(CsrfGuard),
			Err(status) => Outcome::Error((status, ())),
		}
	}
}

/// Validate Origin or Referer against the configured allowlist for mutating requests.
pub fn validate_csrf_origin(request: &Request<'_>, allowed: &[String]) -> Result<(), Status> {
	match request.method() {
		Method::Post | Method::Put | Method::Patch | Method::Delete => {}
		_ => return Ok(()),
	}

	if let Some(origin) = request.headers().get_one("Origin") {
		if allowed.iter().any(|entry| entry == origin) {
			return Ok(());
		}
		return Err(Status::Forbidden);
	}

	if let Some(referer) = request.headers().get_one("Referer") {
		if let Some(origin) = referer_origin(referer)
			&& allowed.iter().any(|entry| entry == &origin)
		{
			return Ok(());
		}
		return Err(Status::Forbidden);
	}

	Err(Status::Forbidden)
}

fn referer_origin(referer: &str) -> Option<String> {
	let without_fragment = referer.split('#').next()?;
	let scheme_end = without_fragment.find("://")?;
	let scheme = &without_fragment[..scheme_end];
	let rest = &without_fragment[scheme_end + 3..];
	let path_start = rest.find('/').unwrap_or(rest.len());
	let authority = &rest[..path_start];
	Some(format!("{}://{}", scheme, authority))
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn referer_origin_parses_url() {
		assert_eq!(
			referer_origin("https://kether.pl/binds?page=1"),
			Some("https://kether.pl".to_string())
		);
		assert_eq!(
			referer_origin("http://localhost:3000/app"),
			Some("http://localhost:3000".to_string())
		);
	}
}
