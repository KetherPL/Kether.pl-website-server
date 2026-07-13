// SPDX-License-Identifier: GPL-3.0-only

use reqwest::Client;
use rocket::serde::{Deserialize, Serialize};

use super::daemon_client::with_daemon_auth;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminInstallMapRequest {
    pub mode: String,
    pub input: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminInstallMapResponse {
    pub map_id: u64,
    pub resolved_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResolvedInstallTarget {
    Workshop { workshop_id: u64 },
    Url { url: String },
    L4d2Center { name: String },
}

#[derive(Debug, Clone)]
pub enum ResolveInstallError {
    EmptyInput,
    InvalidMode(String),
    InvalidInput(String),
}

impl std::fmt::Display for ResolveInstallError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::EmptyInput => write!(f, "Install input is required"),
            Self::InvalidMode(mode) => write!(f, "Invalid install mode '{mode}'"),
            Self::InvalidInput(message) => write!(f, "{message}"),
        }
    }
}

fn trim_optional_name(name: &Option<String>) -> Option<String> {
    name.as_ref()
        .map(|value| value.trim())
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub fn extract_workshop_id(input: &str) -> Option<u64> {
    let trimmed = input.trim();
    if trimmed.chars().all(|ch| ch.is_ascii_digit()) {
        return trimmed.parse().ok();
    }

    let lower = trimmed.to_lowercase();
    if !lower.contains("steamcommunity.com") {
        return None;
    }

    for segment in trimmed.split(&['?', '&', '#'][..]) {
        if let Some(id) = segment.strip_prefix("id=").or_else(|| segment.strip_prefix("ID=")) {
            if let Ok(parsed) = id.parse::<u64>() {
                return Some(parsed);
            }
        }
    }

    None
}

fn is_http_url(input: &str) -> bool {
    let lower = input.trim().to_lowercase();
    lower.starts_with("http://") || lower.starts_with("https://")
}

fn validate_http_url(input: &str) -> Result<String, ResolveInstallError> {
    let trimmed = input.trim();
    if !is_http_url(trimmed) {
        return Err(ResolveInstallError::InvalidInput(
            "Expected an HTTP or HTTPS URL".to_string(),
        ));
    }
    if trimmed.len() > 2048 {
        return Err(ResolveInstallError::InvalidInput(
            "URL too long (max 2048 characters)".to_string(),
        ));
    }
    Ok(trimmed.to_string())
}

fn resolve_workshop_input(input: &str) -> Result<ResolvedInstallTarget, ResolveInstallError> {
    if let Some(workshop_id) = extract_workshop_id(input) {
        return Ok(ResolvedInstallTarget::Workshop { workshop_id });
    }

    Err(ResolveInstallError::InvalidInput(
        "Expected a workshop ID or Steam Workshop URL".to_string(),
    ))
}

pub fn resolve_install_target(
    mode: &str,
    input: &str,
) -> Result<ResolvedInstallTarget, ResolveInstallError> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return Err(ResolveInstallError::EmptyInput);
    }

    match mode.trim().to_lowercase().as_str() {
        "workshop" => resolve_workshop_input(trimmed),
        "sirplease" => Ok(ResolvedInstallTarget::Url {
            url: validate_http_url(trimmed)?,
        }),
        "l4d2center" => Ok(ResolvedInstallTarget::L4d2Center {
            name: trimmed.to_string(),
        }),
        "auto" => {
            if let Some(workshop_id) = extract_workshop_id(trimmed) {
                return Ok(ResolvedInstallTarget::Workshop { workshop_id });
            }
            if is_http_url(trimmed) {
                return Ok(ResolvedInstallTarget::Url {
                    url: validate_http_url(trimmed)?,
                });
            }
            Err(ResolveInstallError::InvalidInput(
                "Could not detect install source. Use a workshop ID, Steam Workshop URL, or download URL."
                    .to_string(),
            ))
        }
        other => Err(ResolveInstallError::InvalidMode(other.to_string())),
    }
}

pub fn resolved_mode_label(target: &ResolvedInstallTarget) -> &'static str {
    match target {
        ResolvedInstallTarget::Workshop { .. } => "workshop",
        ResolvedInstallTarget::Url { .. } => "url",
        ResolvedInstallTarget::L4d2Center { .. } => "l4d2center",
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct DaemonInstallMapRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    url: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    workshop_id: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    name: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct DaemonL4d2CenterInstallRequest {
    name: String,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct DaemonApiEnvelope<T> {
    success: bool,
    data: Option<T>,
    error: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct DaemonMapEntryResponse {
    id: u64,
}

pub async fn proxy_daemon_install_map(
    client: &Client,
    daemon_url: &str,
    daemon_api_key: Option<&str>,
    target: &ResolvedInstallTarget,
    name: Option<String>,
) -> Result<u64, String> {
    let base = daemon_url.trim_end_matches('/');
    let url = format!("{base}/api/maps/install");

    let body = match target {
        ResolvedInstallTarget::Workshop { workshop_id } => DaemonInstallMapRequest {
            url: None,
            workshop_id: Some(*workshop_id),
            name,
        },
        ResolvedInstallTarget::Url { url: download_url } => DaemonInstallMapRequest {
            url: Some(download_url.clone()),
            workshop_id: None,
            name,
        },
        ResolvedInstallTarget::L4d2Center { .. } => {
            return Err("Internal error: L4D2Center install must use dedicated endpoint".to_string());
        }
    };

    post_daemon_json(client, &url, daemon_api_key, &body).await
}

pub async fn proxy_daemon_l4d2center_install(
    client: &Client,
    daemon_url: &str,
    daemon_api_key: Option<&str>,
    catalog_name: &str,
) -> Result<u64, String> {
    let base = daemon_url.trim_end_matches('/');
    let url = format!("{base}/api/maps/l4d2center/install");
    let body = DaemonL4d2CenterInstallRequest {
        name: catalog_name.to_string(),
    };

    let response = with_daemon_auth(client.post(&url), daemon_api_key)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("Daemon request failed: {e}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read daemon response: {e}"))?;

    if !status.is_success() {
        return Err(parse_daemon_error(&text).unwrap_or_else(|| {
            format!("Daemon returned HTTP {status}")
        }));
    }

    let envelope: DaemonApiEnvelope<DaemonMapEntryResponse> =
        rocket::serde::json::from_str(&text)
            .map_err(|e| format!("Failed to parse daemon response: {e}"))?;

    if !envelope.success {
        return Err(envelope
            .error
            .unwrap_or_else(|| "Daemon returned success=false".to_string()));
    }

    envelope
        .data
        .map(|entry| entry.id)
        .ok_or_else(|| "Daemon response missing map entry".to_string())
}

async fn post_daemon_json<T: Serialize>(
    client: &Client,
    url: &str,
    daemon_api_key: Option<&str>,
    body: &T,
) -> Result<u64, String> {
    let response = with_daemon_auth(client.post(url), daemon_api_key)
        .json(body)
        .send()
        .await
        .map_err(|e| format!("Daemon request failed: {e}"))?;

    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read daemon response: {e}"))?;

    if !status.is_success() {
        return Err(parse_daemon_error(&text).unwrap_or_else(|| {
            format!("Daemon returned HTTP {status}")
        }));
    }

    let envelope: DaemonApiEnvelope<u64> = rocket::serde::json::from_str(&text)
        .map_err(|e| format!("Failed to parse daemon response: {e}"))?;

    if !envelope.success {
        return Err(envelope
            .error
            .unwrap_or_else(|| "Daemon returned success=false".to_string()));
    }

    envelope
        .data
        .ok_or_else(|| "Daemon response missing map id".to_string())
}

fn parse_daemon_error(body: &str) -> Option<String> {
    let envelope: DaemonApiEnvelope<()> = rocket::serde::json::from_str(body).ok()?;
    envelope.error
}

pub fn validate_optional_install_name(name: &Option<String>) -> Result<Option<String>, String> {
    let trimmed = trim_optional_name(name);
    if let Some(value) = trimmed.as_ref()
        && value.len() > 255
    {
        return Err("Map name too long (max 255 characters)".to_string());
    }
    Ok(trimmed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn mock_daemon() -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
        let address = listener.local_addr().expect("mock address");
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let mut request = vec![0_u8; 16 * 1024];
            let read = stream.read(&mut request).await.expect("read request");
            request.truncate(read);
            let body = r#"{"success":true,"data":42,"error":null}"#;
            let response = format!(
                "HTTP/1.1 200 OK\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
                body.len()
            );
            stream
                .write_all(response.as_bytes())
                .await
                .expect("write response");
            String::from_utf8_lossy(&request).into_owned()
        });
        (format!("http://{address}"), handle)
    }

    #[test]
    fn resolve_auto_workshop_id() {
        let target = resolve_install_target("auto", "381419931").unwrap();
        assert_eq!(
            target,
            ResolvedInstallTarget::Workshop {
                workshop_id: 381419931
            }
        );
    }

    #[test]
    fn resolve_auto_steam_url() {
        let target = resolve_install_target(
            "auto",
            "https://steamcommunity.com/sharedfiles/filedetails/?id=381419931",
        )
        .unwrap();
        assert_eq!(
            target,
            ResolvedInstallTarget::Workshop {
                workshop_id: 381419931
            }
        );
    }

    #[test]
    fn resolve_auto_download_url() {
        let target = resolve_install_target(
            "auto",
            "https://sirplease.vercel.app/downloads/maps/map.zip",
        )
        .unwrap();
        assert!(matches!(target, ResolvedInstallTarget::Url { .. }));
    }

    #[test]
    fn resolve_workshop_mode_rejects_plain_url() {
        let err = resolve_install_target("workshop", "https://example.com/map.zip").unwrap_err();
        assert!(matches!(err, ResolveInstallError::InvalidInput(_)));
    }

    #[test]
    fn resolve_l4d2center_mode() {
        let target = resolve_install_target("l4d2center", "Wide Box").unwrap();
        assert_eq!(
            target,
            ResolvedInstallTarget::L4d2Center {
                name: "Wide Box".to_string()
            }
        );
    }

    #[test]
    fn resolve_invalid_mode() {
        let err = resolve_install_target("unknown", "123").unwrap_err();
        assert!(matches!(err, ResolveInstallError::InvalidMode(_)));
    }

    #[tokio::test]
    async fn install_sends_configured_bearer_token() {
        let (url, request) = mock_daemon().await;
        let target = ResolvedInstallTarget::Workshop { workshop_id: 123 };

        proxy_daemon_install_map(
            &Client::new(),
            &url,
            Some("daemon-secret"),
            &target,
            None,
        )
        .await
        .expect("install");

        assert!(request
            .await
            .expect("request task")
            .contains("authorization: Bearer daemon-secret"));
    }

    #[tokio::test]
    async fn install_omits_bearer_token_when_unconfigured() {
        let (url, request) = mock_daemon().await;
        let target = ResolvedInstallTarget::Workshop { workshop_id: 123 };

        proxy_daemon_install_map(&Client::new(), &url, None, &target, None)
            .await
            .expect("install");

        assert!(!request
            .await
            .expect("request task")
            .to_ascii_lowercase()
            .contains("authorization:"));
    }
}
