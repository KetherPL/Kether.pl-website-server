// SPDX-License-Identifier: GPL-3.0-only

use reqwest::Client;
use rocket::serde::{Deserialize, Serialize};

use super::daemon_client::with_daemon_auth;
use super::models::DaemonMapEntry;

#[derive(Debug, Clone)]
pub enum MapUpdateOutcome {
    Updated(DaemonMapEntry),
    UpToDate,
    Failed(String),
}

#[derive(Debug, Serialize)]
#[serde(crate = "rocket::serde")]
struct DaemonUpdateRequest {
    map_id: Option<u64>,
    force: bool,
    check_only: bool,
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
struct MapOperationFailure {
    map_id: u64,
    error: String,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct WorkshopUpdateReport {
    updated: Vec<DaemonMapEntry>,
    skipped: usize,
    failed: Vec<MapOperationFailure>,
    not_workshop: usize,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct L4d2CenterUpdateReport {
    updated: Vec<DaemonMapEntry>,
    skipped: usize,
    failed: Vec<MapOperationFailure>,
    not_l4d2center: usize,
}

pub async fn proxy_daemon_get_map(
    client: &Client,
    daemon_url: &str,
    daemon_api_key: Option<&str>,
    id: u64,
) -> Result<DaemonMapEntry, String> {
    let url = format!(
        "{}/api/maps/{id}",
        daemon_url.trim_end_matches('/')
    );
    let response = with_daemon_auth(client.get(&url), daemon_api_key)
        .send()
        .await
        .map_err(|e| format!("Daemon request failed: {e}"))?;
    parse_daemon_response(response).await
}

pub async fn proxy_daemon_uninstall_map(
    client: &Client,
    daemon_url: &str,
    daemon_api_key: Option<&str>,
    id: u64,
) -> Result<(), String> {
    let url = format!(
        "{}/api/maps/uninstall/{id}",
        daemon_url.trim_end_matches('/')
    );
    let response = with_daemon_auth(client.post(&url), daemon_api_key)
        .send()
        .await
        .map_err(|e| format!("Daemon request failed: {e}"))?;
    parse_daemon_empty_response(response).await
}

pub async fn proxy_daemon_workshop_update(
    client: &Client,
    daemon_url: &str,
    daemon_api_key: Option<&str>,
    id: u64,
) -> Result<MapUpdateOutcome, String> {
    let url = format!(
        "{}/api/maps/workshop/update",
        daemon_url.trim_end_matches('/')
    );
    let report: WorkshopUpdateReport =
        post_update_request(client, &url, daemon_api_key, id).await?;
    Ok(classify_update_report(
        id,
        report.updated,
        report.skipped,
        report.failed,
        report.not_workshop,
        "workshop",
    ))
}

pub async fn proxy_daemon_l4d2center_update(
    client: &Client,
    daemon_url: &str,
    daemon_api_key: Option<&str>,
    id: u64,
) -> Result<MapUpdateOutcome, String> {
    let url = format!(
        "{}/api/maps/l4d2center/update",
        daemon_url.trim_end_matches('/')
    );
    let report: L4d2CenterUpdateReport =
        post_update_request(client, &url, daemon_api_key, id).await?;
    Ok(classify_update_report(
        id,
        report.updated,
        report.skipped,
        report.failed,
        report.not_l4d2center,
        "L4D2Center",
    ))
}

async fn post_update_request<T: for<'de> Deserialize<'de>>(
    client: &Client,
    url: &str,
    daemon_api_key: Option<&str>,
    id: u64,
) -> Result<T, String> {
    let response = with_daemon_auth(client.post(url), daemon_api_key)
        .json(&DaemonUpdateRequest {
            map_id: Some(id),
            force: false,
            check_only: false,
        })
        .send()
        .await
        .map_err(|e| format!("Daemon request failed: {e}"))?;
    parse_daemon_response(response).await
}

async fn parse_daemon_response<T: for<'de> Deserialize<'de>>(
    response: reqwest::Response,
) -> Result<T, String> {
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read daemon response: {e}"))?;

    let envelope: DaemonApiEnvelope<T> = rocket::serde::json::from_str(&text)
        .map_err(|e| format!("Failed to parse daemon response: {e}"))?;

    if !status.is_success() || !envelope.success {
        return Err(envelope
            .error
            .unwrap_or_else(|| format!("Daemon returned HTTP {status}")));
    }

    envelope
        .data
        .ok_or_else(|| "Daemon response missing data".to_string())
}

async fn parse_daemon_empty_response(response: reqwest::Response) -> Result<(), String> {
    let status = response.status();
    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read daemon response: {e}"))?;
    let envelope: DaemonApiEnvelope<()> = rocket::serde::json::from_str(&text)
        .map_err(|e| format!("Failed to parse daemon response: {e}"))?;

    if !status.is_success() || !envelope.success {
        return Err(envelope
            .error
            .unwrap_or_else(|| format!("Daemon returned HTTP {status}")));
    }
    Ok(())
}

fn classify_update_report(
    map_id: u64,
    updated: Vec<DaemonMapEntry>,
    skipped: usize,
    failed: Vec<MapOperationFailure>,
    unsupported: usize,
    source_label: &str,
) -> MapUpdateOutcome {
    if let Some(entry) = updated.into_iter().find(|entry| entry.id == map_id) {
        return MapUpdateOutcome::Updated(entry);
    }

    if let Some(failure) = failed
        .iter()
        .find(|failure| failure.map_id == map_id)
        .or_else(|| failed.first())
    {
        return MapUpdateOutcome::Failed(failure.error.clone());
    }

    if unsupported > 0 {
        return MapUpdateOutcome::Failed(format!(
            "Map is not a {source_label} addon"
        ));
    }

    if skipped > 0 {
        return MapUpdateOutcome::UpToDate;
    }

    MapUpdateOutcome::Failed("Daemon returned no update result".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::io::{AsyncReadExt, AsyncWriteExt};
    use tokio::net::TcpListener;

    async fn mock_daemon(
        status: &str,
        body: String,
    ) -> (String, tokio::task::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").await.expect("bind mock");
        let address = listener.local_addr().expect("mock address");
        let status = status.to_string();
        let handle = tokio::spawn(async move {
            let (mut stream, _) = listener.accept().await.expect("accept request");
            let mut request = vec![0_u8; 16 * 1024];
            let read = stream.read(&mut request).await.expect("read request");
            request.truncate(read);
            let response = format!(
                "HTTP/1.1 {status}\r\ncontent-type: application/json\r\ncontent-length: {}\r\nconnection: close\r\n\r\n{body}",
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

    fn map_json(id: u64) -> String {
        format!(
            r#"{{
                "id": {id},
                "name": "Test Addon",
                "source_url": "",
                "source_kind": "workshop",
                "workshop_id": 123,
                "installed_path": "workshop/test.vpk",
                "installed_at": "2026-07-13T12:00:00Z",
                "workshop_updated_at": "2026-07-12T12:00:00Z",
                "version": "1",
                "checksum": "abc",
                "checksum_kind": "md5"
            }}"#
        )
    }

    #[tokio::test]
    async fn get_map_proxies_id_and_parses_detail() {
        let body = format!(
            r#"{{"success":true,"data":{},"error":null}}"#,
            map_json(42)
        );
        let (url, request) = mock_daemon("200 OK", body).await;

        let map = proxy_daemon_get_map(&Client::new(), &url, None, 42)
            .await
            .expect("map detail");
        assert_eq!(map.id, 42);
        assert_eq!(map.installed_path, "workshop/test.vpk");
        let request = request.await.expect("request task");
        assert!(request.starts_with("GET /api/maps/42 "));
        assert!(!request.to_ascii_lowercase().contains("authorization:"));
    }

    #[tokio::test]
    async fn uninstall_posts_to_daemon() {
        let (url, request) = mock_daemon(
            "200 OK",
            r#"{"success":true,"data":null,"error":null}"#.to_string(),
        )
        .await;

        proxy_daemon_uninstall_map(&Client::new(), &url, None, 7)
            .await
            .expect("uninstall");
        assert!(request
            .await
            .expect("request task")
            .starts_with("POST /api/maps/uninstall/7 "));
    }

    #[tokio::test]
    async fn workshop_update_reports_updated_entry() {
        let body = format!(
            r#"{{"success":true,"data":{{"updated":[{}],"available":[],"skipped":0,"failed":[],"not_workshop":0}},"error":null}}"#,
            map_json(9)
        );
        let (url, request) = mock_daemon("200 OK", body).await;

        let outcome = proxy_daemon_workshop_update(&Client::new(), &url, None, 9)
            .await
            .expect("update");
        assert!(matches!(outcome, MapUpdateOutcome::Updated(entry) if entry.id == 9));
        let request = request.await.expect("request task");
        assert!(request.starts_with("POST /api/maps/workshop/update "));
        assert!(request.contains(r#""map_id":9"#));
        assert!(request.contains(r#""check_only":false"#));
    }

    #[tokio::test]
    async fn l4d2center_update_reports_up_to_date() {
        let body = r#"{"success":true,"data":{"updated":[],"available":[],"skipped":1,"failed":[],"not_l4d2center":0},"error":null}"#;
        let (url, _) = mock_daemon("200 OK", body.to_string()).await;

        let outcome = proxy_daemon_l4d2center_update(&Client::new(), &url, None, 11)
            .await
            .expect("update check");
        assert!(matches!(outcome, MapUpdateOutcome::UpToDate));
    }

    #[tokio::test]
    async fn get_map_sends_configured_bearer_token() {
        let body = format!(
            r#"{{"success":true,"data":{},"error":null}}"#,
            map_json(42)
        );
        let (url, request) = mock_daemon("200 OK", body).await;

        proxy_daemon_get_map(&Client::new(), &url, Some("daemon-secret"), 42)
            .await
            .expect("map detail");

        let request = request.await.expect("request task");
        assert!(request.contains("authorization: Bearer daemon-secret"));
    }
}

