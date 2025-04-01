// SPDX-License-Identifier: GPL-3.0-only

use rocket::response::Redirect;
use rocket::{get, routes, response::content::RawText, http::Status, Request};
use rocket::serde::{Serialize, Deserialize};
use std::fs::OpenOptions;
use std::io::{self, Write};
use std::net::IpAddr;
use std::path::Path;
use rocket::request::{FromRequest, Outcome};

#[derive(Serialize, Deserialize, Debug)]
#[serde(crate = "rocket::serde")]
pub struct SimpleResponse {
    message: String,
}

// Custom guard to extract the client's IP address and the full requested path
pub struct ClientInfo {
    pub ip: Option<IpAddr>,
    pub path: String,
	pub ua: String,
}

#[rocket::async_trait]
impl<'r> FromRequest<'r> for ClientInfo {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        // List of trusted proxy IPs (add your proxy's IP here)
        let trusted_proxies: Vec<IpAddr> = vec![
            "127.0.0.1".parse().unwrap(), // Localhost (if your proxy is on the same machine)
            // Add other trusted proxy IPs here, e.g., "192.168.1.1".parse().unwrap(),
        ];

        let mut ip: Option<IpAddr> = None;

        // Check for X-Forwarded-For header
        if let Some(forwarded_for) = request.headers().get_one("X-Forwarded-For") {
            let ips: Vec<&str> = forwarded_for.split(',').map(|s| s.trim()).collect();
            if let Some(first_ip_str) = ips.first() {
                if let Ok(first_ip) = first_ip_str.parse::<IpAddr>() {
                    // Check if the last IP in the chain is a trusted proxy
                    if let Some(last_ip_str) = ips.last() {
                        if let Ok(last_ip) = last_ip_str.parse::<IpAddr>() {
                            if trusted_proxies.contains(&last_ip) {
                                ip = Some(first_ip);
                            }
                        }
                    }
                }
            }
        }

        // Check for X-Real-IP header if X-Forwarded-For is not present or not trusted
        if ip.is_none() {
            if let Some(real_ip_str) = request.headers().get_one("X-Real-IP") {
                if let Ok(real_ip) = real_ip_str.parse::<IpAddr>() {
                    ip = Some(real_ip);
                }
            }
        }

        // Fallback to client_ip if no trusted header is found
        if ip.is_none() {
            // Only use client_ip if the direct connection is from a trusted proxy
            if let Some(client_ip) = request.client_ip() {
                if trusted_proxies.contains(&client_ip) {
                    ip = Some(client_ip);
                }
            }
        }

        // Get the full requested path
        let path = request.uri().path().to_string();
		let ua = request.headers().get_one("User-Agent").unwrap_or("Unknown").to_string();

        Outcome::Success(ClientInfo { ip, path, ua })
    }
}

fn log_suspect(info: &ClientInfo) -> io::Result<()> {
    let log_path = Path::new("suspects.log");
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(log_path)?;

    if let Some(ip) = info.ip {
        writeln!(file, "Suspect IP: {} | Path: {} | UA: {}", ip, info.path, info.ua)?;
    } else {
        writeln!(file, "Suspect IP: Unknown | Path: {} | UA: {}", info.path, info.ua)?;
    }

    Ok(())
}

#[get("/phpmyadmin/<_..>")]
pub fn phpmyadmin(client_info: ClientInfo) -> Result<RawText<String>, Status> {
    if let Err(e) = log_suspect(&client_info) {
        eprintln!("Failed to log suspect IP: {}", e);
        return Err(Status::InternalServerError);
    }
    Ok(RawText("Иди нахуй!".to_string()))
}

#[get("/wordpress/<_..>")]
pub fn wordpress(client_info: ClientInfo) -> Result<RawText<String>, Status> {
    if let Err(e) = log_suspect(&client_info) {
        eprintln!("Failed to log suspect IP: {}", e);
        return Err(Status::InternalServerError);
    }
    Ok(RawText("Иди нахуй!".to_string()))
}

#[get("/.git/<_..>")]
pub fn dotgit(client_info: ClientInfo) -> Result<RawText<String>, Status> {
    if let Err(e) = log_suspect(&client_info) {
        eprintln!("Failed to log suspect IP: {}", e);
        return Err(Status::InternalServerError);
    }
    Ok(RawText("Иди нахуй!".to_string()))
}

#[get("/wp-admin/<_..>")]
pub fn wpadmin(client_info: ClientInfo) -> Result<RawText<String>, Status> {
    if let Err(e) = log_suspect(&client_info) {
        eprintln!("Failed to log suspect IP: {}", e);
        return Err(Status::InternalServerError);
    }
    Ok(RawText("Иди нахуй!".to_string()))
}

// --- Redirect from root path to our frontend
#[get("/")]
pub fn redirect_to_kether() -> Redirect {
    Redirect::to("https://kether.pl")
}

pub fn mount_sat_specific_routes() -> Vec<rocket::Route> {
    routes![
        wordpress,
        phpmyadmin,
        dotgit,
        wpadmin,
		redirect_to_kether,
    ]
}
