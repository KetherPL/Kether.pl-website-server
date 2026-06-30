// SPDX-License-Identifier: GPL-3.0-only

use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use std::net::IpAddr;

/// Client IP extracted from the incoming request.
pub struct ClientIp(pub IpAddr);

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientIp {
	type Error = ();

	async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
		match request.client_ip() {
			Some(ip) => Outcome::Success(ClientIp(ip)),
			None => Outcome::Error((Status::BadRequest, ())),
		}
	}
}
