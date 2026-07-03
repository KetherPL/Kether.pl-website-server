// SPDX-License-Identifier: GPL-3.0-only

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use reqwest::Client;
use rocket::serde::{Deserialize, Serialize};

use super::models::{DaemonMapEntry, DaemonSourceKind};

pub const STEAM_PUBLISHED_FILE_DETAILS_URL: &str =
    "https://api.steampowered.com/ISteamRemoteStorage/GetPublishedFileDetails/v1/";

const STEAM_BATCH_LIMIT: usize = 100;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(crate = "rocket::serde")]
pub struct PreviewCacheEntry {
    pub preview_url: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workshop_updated_at: Option<DateTime<Utc>>,
    pub fetched_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
#[serde(crate = "rocket::serde")]
pub struct WorkshopPreviewCache {
    #[serde(flatten)]
    entries: HashMap<String, PreviewCacheEntry>,
}

impl WorkshopPreviewCache {
    pub fn get(&self, workshop_id: u64) -> Option<&PreviewCacheEntry> {
        self.entries.get(&workshop_id.to_string())
    }

    pub fn preview_url(&self, workshop_id: u64) -> Option<&str> {
        self.get(workshop_id)
            .map(|entry| entry.preview_url.as_str())
            .filter(|url| !url.is_empty())
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn insert(&mut self, workshop_id: u64, entry: PreviewCacheEntry) {
        self.entries.insert(workshop_id.to_string(), entry);
    }

    fn retain_workshop_ids(&mut self, active_ids: &HashSet<u64>) {
        self.entries
            .retain(|key, _| key.parse::<u64>().ok().is_some_and(|id| active_ids.contains(&id)));
    }
}

pub fn preview_cache_path_from_registry(registry_path: &Path) -> PathBuf {
    registry_path
        .parent()
        .map(|parent| parent.join("workshop_previews.json"))
        .unwrap_or_else(|| PathBuf::from("workshop_previews.json"))
}

pub fn load_preview_cache(path: &Path) -> Result<WorkshopPreviewCache, String> {
    if !path.exists() {
        return Ok(WorkshopPreviewCache::default());
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read preview cache {}: {}", path.display(), e))?;

    rocket::serde::json::from_str(&content)
        .map_err(|e| format!("Failed to parse preview cache {}: {}", path.display(), e))
}

pub fn save_preview_cache(path: &Path, cache: &WorkshopPreviewCache) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create preview cache directory: {}", e))?;
    }

    let json = rocket::serde::json::to_pretty_string(cache)
        .map_err(|e| format!("Failed to serialize preview cache: {}", e))?;

    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, json)
        .map_err(|e| format!("Failed to write temp preview cache: {}", e))?;
    std::fs::rename(&temp_path, path)
        .map_err(|e| format!("Failed to rename preview cache file: {}", e))?;

    Ok(())
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct SteamApiResponse {
    response: SteamApiResponseBody,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct SteamApiResponseBody {
    #[serde(default)]
    publishedfiledetails: Vec<SteamPublishedFile>,
}

#[derive(Debug, Deserialize)]
#[serde(crate = "rocket::serde")]
struct SteamPublishedFile {
    publishedfileid: String,
    #[serde(default)]
    result: u32,
    #[serde(default)]
    preview_url: Option<String>,
}

pub fn parse_preview_urls_from_response(body: &str) -> Result<HashMap<u64, String>, String> {
    let parsed: SteamApiResponse = rocket::serde::json::from_str(body)
        .map_err(|e| format!("Failed to parse Steam API response: {}", e))?;

    let mut previews = HashMap::new();
    for item in parsed.response.publishedfiledetails {
        if item.result != 1 {
            continue;
        }

        let Some(url) = item
            .preview_url
            .as_deref()
            .map(str::trim)
            .filter(|url| !url.is_empty())
        else {
            continue;
        };

        let Ok(workshop_id) = item.publishedfileid.parse::<u64>() else {
            continue;
        };

        previews.insert(workshop_id, url.to_string());
    }

    Ok(previews)
}

fn build_steam_request_body(ids: &[u64], api_key: Option<&str>) -> String {
    let mut body = format!("itemcount={}", ids.len());
    for (index, id) in ids.iter().enumerate() {
        body.push_str(&format!("&publishedfileids[{index}]={id}"));
    }
    if let Some(key) = api_key.map(str::trim).filter(|key| !key.is_empty()) {
        body.push_str("&key=");
        body.push_str(key);
    }
    body
}

pub async fn fetch_preview_urls(
    client: &Client,
    api_url: &str,
    ids: &[u64],
    api_key: Option<&str>,
) -> Result<HashMap<u64, String>, String> {
    if ids.is_empty() {
        return Ok(HashMap::new());
    }

    let mut all_previews = HashMap::new();

    for chunk in ids.chunks(STEAM_BATCH_LIMIT) {
        let body = build_steam_request_body(chunk, api_key);
        let response = client
            .post(api_url)
            .header("Content-Type", "application/x-www-form-urlencoded")
            .body(body)
            .send()
            .await
            .map_err(|e| format!("Steam Web API request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Steam Web API returned HTTP {}", response.status()));
        }

        let text = response
            .text()
            .await
            .map_err(|e| format!("Failed to read Steam API response: {}", e))?;

        let chunk_previews = parse_preview_urls_from_response(&text)?;
        all_previews.extend(chunk_previews);
    }

    Ok(all_previews)
}

struct WorkshopRegistryInfo {
    workshop_id: u64,
    workshop_updated_at: Option<DateTime<Utc>>,
}

fn workshop_maps_from_registry(maps: &[DaemonMapEntry]) -> Vec<WorkshopRegistryInfo> {
    maps.iter()
        .filter(|entry| entry.source_kind == DaemonSourceKind::Workshop)
        .filter_map(|entry| {
            entry.workshop_id.map(|workshop_id| WorkshopRegistryInfo {
                workshop_id,
                workshop_updated_at: entry.workshop_updated_at,
            })
        })
        .collect()
}

fn needs_refresh(
    cache: &WorkshopPreviewCache,
    workshop_id: u64,
    workshop_updated_at: Option<DateTime<Utc>>,
) -> bool {
    match cache.get(workshop_id) {
        None => true,
        Some(entry) => entry.workshop_updated_at != workshop_updated_at,
    }
}

pub async fn refresh_preview_cache(
    client: &Client,
    api_url: &str,
    cache_path: &Path,
    maps: &[DaemonMapEntry],
    api_key: Option<&str>,
) -> Result<(), String> {
    let workshop_maps = workshop_maps_from_registry(maps);
    let active_ids: HashSet<u64> = workshop_maps
        .iter()
        .map(|info| info.workshop_id)
        .collect();

    let mut cache = load_preview_cache(cache_path)?;
    cache.retain_workshop_ids(&active_ids);

    let ids_to_fetch: Vec<u64> = workshop_maps
        .iter()
        .filter(|info| needs_refresh(&cache, info.workshop_id, info.workshop_updated_at))
        .map(|info| info.workshop_id)
        .collect();

    if !ids_to_fetch.is_empty() {
        match fetch_preview_urls(client, api_url, &ids_to_fetch, api_key).await {
            Ok(fetched) => {
                let now = Utc::now();
                for info in &workshop_maps {
                    if let Some(preview_url) = fetched.get(&info.workshop_id) {
                        cache.insert(
                            info.workshop_id,
                            PreviewCacheEntry {
                                preview_url: preview_url.clone(),
                                workshop_updated_at: info.workshop_updated_at,
                                fetched_at: now,
                            },
                        );
                    }
                }
            }
            Err(error) => {
                eprintln!("Workshop preview fetch failed (non-fatal): {}", error);
            }
        }
    }

    save_preview_cache(cache_path, &cache)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    const MOCK_STEAM_RESPONSE: &str = r#"{
        "response": {
            "result": 1,
            "resultcount": 2,
            "publishedfiledetails": [
                {
                    "publishedfileid": "381419931",
                    "result": 1,
                    "preview_url": "https://images.steamusercontent.com/ugc/539639065356290110/D4A3FD2E6B8D9260D57E320155B43C0F4C1B65BF/"
                },
                {
                    "publishedfileid": "999999999999",
                    "result": 9
                }
            ]
        }
    }"#;

    fn sample_workshop_entry(workshop_id: u64, updated: Option<DateTime<Utc>>) -> DaemonMapEntry {
        DaemonMapEntry {
            id: 1,
            name: "Workshop Map".to_string(),
            source_url: String::new(),
            source_kind: DaemonSourceKind::Workshop,
            workshop_id: Some(workshop_id),
            installed_path: "map.vpk".to_string(),
            installed_at: Utc::now(),
            workshop_updated_at: updated,
            version: None,
            checksum: None,
            checksum_kind: None,
        }
    }

    #[test]
    fn parse_preview_urls_skips_failed_results() {
        let previews = parse_preview_urls_from_response(MOCK_STEAM_RESPONSE).expect("parse");
        assert_eq!(previews.len(), 1);
        assert!(previews
            .get(&381419931)
            .unwrap()
            .contains("images.steamusercontent.com"));
    }

    #[test]
    fn build_steam_request_body_includes_key_when_present() {
        let body = build_steam_request_body(&[381419931, 1643520526], Some("test-key"));
        assert!(body.contains("itemcount=2"));
        assert!(body.contains("publishedfileids[0]=381419931"));
        assert!(body.contains("publishedfileids[1]=1643520526"));
        assert!(body.contains("key=test-key"));
    }

    #[test]
    fn build_steam_request_body_omits_empty_key() {
        let body = build_steam_request_body(&[381419931], Some("  "));
        assert!(!body.contains("key="));
    }

    #[test]
    fn needs_refresh_when_missing_or_timestamp_changed() {
        let cache = WorkshopPreviewCache::default();
        let ts = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();

        assert!(needs_refresh(&cache, 381419931, None));

        let mut cache_with_entry = WorkshopPreviewCache::default();
        cache_with_entry.insert(
            381419931,
            PreviewCacheEntry {
                preview_url: "https://example.com/preview.jpg".to_string(),
                workshop_updated_at: None,
                fetched_at: Utc::now(),
            },
        );
        assert!(!needs_refresh(&cache_with_entry, 381419931, None));
        assert!(needs_refresh(&cache_with_entry, 381419931, Some(ts)));
    }

    #[test]
    fn save_and_load_preview_cache_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("workshop_previews.json");
        let ts = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();

        let mut cache = WorkshopPreviewCache::default();
        cache.insert(
            381419931,
            PreviewCacheEntry {
                preview_url: "https://images.steamusercontent.com/ugc/test/".to_string(),
                workshop_updated_at: Some(ts),
                fetched_at: ts,
            },
        );

        save_preview_cache(&path, &cache).expect("save");
        let loaded = load_preview_cache(&path).expect("load");

        assert_eq!(loaded.preview_url(381419931), Some("https://images.steamusercontent.com/ugc/test/"));
        assert_eq!(loaded.get(381419931).unwrap().workshop_updated_at, Some(ts));
    }

    #[test]
    fn preview_cache_path_derived_from_registry_path() {
        let registry = PathBuf::from("/data/maps_registry.json");
        assert_eq!(
            preview_cache_path_from_registry(&registry),
            PathBuf::from("/data/workshop_previews.json")
        );
    }

    #[tokio::test]
    async fn fetch_preview_urls_chunks_requests() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let request_count = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&request_count);

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf);
                counter.fetch_add(1, Ordering::SeqCst);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    MOCK_STEAM_RESPONSE.len(),
                    MOCK_STEAM_RESPONSE
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("client");
        let api_url = format!("http://{addr}/steam/GetPublishedFileDetails/v1/");

        let ids: Vec<u64> = (1..=101).collect();
        let previews = fetch_preview_urls(&client, &api_url, &ids, None)
            .await
            .expect("fetch");

        assert_eq!(request_count.load(Ordering::SeqCst), 2);
        assert_eq!(previews.len(), 1);
        assert!(previews.contains_key(&381419931));
    }

    #[tokio::test]
    async fn refresh_preview_cache_fetches_missing_and_prunes_orphans() {
        use std::sync::atomic::{AtomicUsize, Ordering};
        use std::sync::Arc;

        let request_count = Arc::new(AtomicUsize::new(0));
        let counter = Arc::clone(&request_count);

        let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("bind");
        let addr = listener.local_addr().expect("addr");
        std::thread::spawn(move || {
            for mut stream in listener.incoming().flatten() {
                use std::io::{Read, Write};
                let mut buf = [0u8; 8192];
                let _ = stream.read(&mut buf);
                counter.fetch_add(1, Ordering::SeqCst);
                let response = format!(
                    "HTTP/1.1 200 OK\r\nContent-Type: application/json\r\nContent-Length: {}\r\n\r\n{}",
                    MOCK_STEAM_RESPONSE.len(),
                    MOCK_STEAM_RESPONSE
                );
                let _ = stream.write_all(response.as_bytes());
            }
        });

        let dir = tempfile::tempdir().expect("tempdir");
        let cache_path = dir.path().join("workshop_previews.json");

        let mut cache = WorkshopPreviewCache::default();
        cache.insert(
            999_888_777,
            PreviewCacheEntry {
                preview_url: "https://example.com/orphan.jpg".to_string(),
                workshop_updated_at: None,
                fetched_at: Utc::now(),
            },
        );
        save_preview_cache(&cache_path, &cache).expect("seed cache");

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(5))
            .build()
            .expect("client");
        let api_url = format!("http://{addr}/steam/GetPublishedFileDetails/v1/");

        refresh_preview_cache(
            &client,
            &api_url,
            &cache_path,
            &[sample_workshop_entry(381419931, None)],
            None,
        )
        .await
        .expect("refresh");

        let loaded = load_preview_cache(&cache_path).expect("load");
        assert_eq!(loaded.len(), 1);
        assert!(loaded.preview_url(381419931).is_some());
        assert!(loaded.get(999_888_777).is_none());
        assert_eq!(request_count.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn refresh_preview_cache_skips_fetch_when_timestamps_match() {
        let dir = tempfile::tempdir().expect("tempdir");
        let cache_path = dir.path().join("workshop_previews.json");
        let ts = Utc.timestamp_opt(1_700_000_000, 0).single().unwrap();

        let mut cache = WorkshopPreviewCache::default();
        cache.insert(
            381419931,
            PreviewCacheEntry {
                preview_url: "https://images.steamusercontent.com/ugc/cached/".to_string(),
                workshop_updated_at: Some(ts),
                fetched_at: ts,
            },
        );
        save_preview_cache(&cache_path, &cache).expect("seed cache");

        let client = Client::builder()
            .timeout(std::time::Duration::from_secs(1))
            .build()
            .expect("client");

        refresh_preview_cache(
            &client,
            "http://127.0.0.1:1/unreachable",
            &cache_path,
            &[sample_workshop_entry(381419931, Some(ts))],
            None,
        )
        .await
        .expect("refresh");

        let loaded = load_preview_cache(&cache_path).expect("load");
        assert_eq!(
            loaded.preview_url(381419931),
            Some("https://images.steamusercontent.com/ugc/cached/")
        );
    }
}
