// SPDX-License-Identifier: GPL-3.0-only

mod mapping;
mod models;
mod registry_store;
mod workshop_previews;

use std::path::PathBuf;
use std::time::Duration;

use chrono::Utc;
use reqwest::Client;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::serde::json::Json;
use rocket::{get, post, routes, Route, State};

use crate::steam_bot::registry::ConfigHandle;

use mapping::daemon_entry_to_website;
use models::{
    DaemonApiResponse, DaemonMapEntry, MapsDataSource, MapsListResponse, SyncRequest,
    UpdatesResponse,
};
use registry_store::{load_registry, resolve_data_path, save_registry};
use workshop_previews::{
    load_preview_cache, preview_cache_path_from_registry, refresh_preview_cache,
    STEAM_PUBLISHED_FILE_DETAILS_URL,
};

pub struct MapsBridgeState {
    pub registry_path: PathBuf,
    pub daemon_url: String,
    pub stale_after_secs: u64,
    http_client: Client,
    published_file_details_url: String,
}

impl MapsBridgeState {
    pub fn from_config(config: &crate::config::Config, base_path: PathBuf) -> Result<Self, String> {
        let http_client = Client::builder()
            .timeout(Duration::from_secs(15))
            .user_agent(format!(
                "{}/{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(|e| format!("Failed to build HTTP client: {}", e))?;

        Ok(Self {
            registry_path: resolve_data_path(
                &base_path,
                &config.server_daemon_registry_path,
                "maps_registry.json",
            ),
            daemon_url: config.server_daemon_url.clone(),
            stale_after_secs: config.server_daemon_stale_after_secs,
            http_client,
            published_file_details_url: STEAM_PUBLISHED_FILE_DETAILS_URL.to_string(),
        })
    }

    fn preview_cache_path(&self) -> PathBuf {
        preview_cache_path_from_registry(&self.registry_path)
    }

    pub async fn refresh_workshop_previews(
        &self,
        maps: &[DaemonMapEntry],
        api_key: Option<&str>,
    ) {
        if let Err(error) = refresh_preview_cache(
            &self.http_client,
            &self.published_file_details_url,
            &self.preview_cache_path(),
            maps,
            api_key,
        )
        .await
        {
            eprintln!("Workshop preview cache refresh failed (non-fatal): {}", error);
        }
    }

    pub fn verify_sync_token(&self, config: &crate::config::Config, auth_header: Option<&str>) -> bool {
        let expected = config.server_daemon_sync_api_key.trim();
        if expected.is_empty() {
            return false;
        }

        match auth_header {
            Some(value) if value == format!("Bearer {}", expected) => true,
            _ => false,
        }
    }

    pub fn apply_sync(&self, maps: Vec<DaemonMapEntry>) -> Result<(), String> {
        save_registry(&self.registry_path, maps)
    }

    fn registry_is_stale(&self, last_synced_at: Option<chrono::DateTime<Utc>>) -> bool {
        let Some(synced_at) = last_synced_at else {
            return true;
        };

        if self.stale_after_secs == 0 {
            return false;
        }

        let age = Utc::now().signed_duration_since(synced_at);
        age.num_seconds() > self.stale_after_secs as i64
    }

    async fn fetch_maps_from_daemon(&self) -> Result<Vec<DaemonMapEntry>, String> {
        if self.daemon_url.trim().is_empty() {
            return Err("Daemon URL not configured".to_string());
        }

        let url = format!(
            "{}/api/maps",
            self.daemon_url.trim_end_matches('/')
        );

        let response = self
            .http_client
            .get(&url)
            .send()
            .await
            .map_err(|e| format!("Daemon request failed: {}", e))?;

        if !response.status().is_success() {
            return Err(format!("Daemon returned HTTP {}", response.status()));
        }

        let body: DaemonApiResponse = response
            .json()
            .await
            .map_err(|e| format!("Failed to parse daemon response: {}", e))?;

        if !body.success {
            return Err(body
                .error
                .unwrap_or_else(|| "Daemon returned success=false".to_string()));
        }

        body.data.ok_or_else(|| "Daemon response missing data".to_string())
    }

    fn empty_stale_response() -> MapsListResponse {
        MapsListResponse {
            maps: Vec::new(),
            stale: true,
            source: MapsDataSource::RegistryStale,
        }
    }

    pub async fn build_maps_response(
        &self,
        steam_api_key: Option<&str>,
    ) -> Result<MapsListResponse, String> {
        let mut registry = load_registry(&self.registry_path)?;

        let mut refreshed = false;
        if registry.maps.is_empty() || self.registry_is_stale(registry.last_synced_at) {
            match self.fetch_maps_from_daemon().await {
                Ok(maps) => {
                    save_registry(&self.registry_path, maps.clone())?;
                    self.refresh_workshop_previews(&maps, steam_api_key).await;
                    registry.maps = maps;
                    registry.last_synced_at = Some(Utc::now());
                    refreshed = true;
                }
                Err(_) if !registry.maps.is_empty() => {
                    // Keep cached registry; mark stale below.
                }
                Err(_) => {
                    return Ok(Self::empty_stale_response());
                }
            }
        }

        if registry.maps.is_empty() {
            return Ok(Self::empty_stale_response());
        }

        let stale = !refreshed
            && (self.registry_is_stale(registry.last_synced_at) || registry.last_synced_at.is_none());

        let preview_cache = load_preview_cache(&self.preview_cache_path())?;
        let maps = registry
            .maps
            .iter()
            .map(|entry| daemon_entry_to_website(entry, &preview_cache))
            .collect();

        Ok(MapsListResponse {
            maps,
            stale,
            source: if stale {
                MapsDataSource::RegistryStale
            } else {
                MapsDataSource::Registry
            },
        })
    }
}

pub struct DaemonSyncAuth;

#[rocket::async_trait]
impl<'r> FromRequest<'r> for DaemonSyncAuth {
    type Error = ();

    async fn from_request(request: &'r Request<'_>) -> Outcome<Self, Self::Error> {
        let config_handle = match request.guard::<&State<ConfigHandle>>().await {
            Outcome::Success(handle) => handle,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        let bridge = match request.guard::<&State<MapsBridgeState>>().await {
            Outcome::Success(state) => state,
            _ => return Outcome::Error((Status::InternalServerError, ())),
        };

        let config = config_handle.read().ok().map(|cfg| cfg.clone());
        let Some(config) = config else {
            return Outcome::Error((Status::InternalServerError, ()));
        };

        let auth_header = request
            .headers()
            .get_one("Authorization");

        if bridge.verify_sync_token(&config, auth_header) {
            Outcome::Success(DaemonSyncAuth)
        } else {
            Outcome::Error((Status::Unauthorized, ()))
        }
    }
}

#[post("/registry/sync", data = "<body>")]
pub async fn sync_registry(
    _auth: DaemonSyncAuth,
    bridge: &State<MapsBridgeState>,
    config_handle: &State<ConfigHandle>,
    body: Json<SyncRequest>,
) -> Result<Status, Status> {
    let maps = body.maps.clone();
    bridge.apply_sync(maps.clone()).map_err(|e| {
        eprintln!("Registry sync failed: {}", e);
        Status::InternalServerError
    })?;

    let api_key = config_handle
        .read()
        .ok()
        .map(|config| config.steam_web_api_key.clone());

    let key_ref = api_key.as_deref();
    bridge.refresh_workshop_previews(&maps, key_ref).await;

    Ok(Status::Ok)
}

#[get("/registry/updates")]
pub fn registry_updates(_auth: DaemonSyncAuth) -> Json<UpdatesResponse> {
    Json(UpdatesResponse {
        updates: Vec::new(),
    })
}

#[get("/maps")]
pub async fn list_maps(
    bridge: &State<MapsBridgeState>,
    config_handle: &State<ConfigHandle>,
) -> Result<Json<MapsListResponse>, Status> {
    let api_key = config_handle
        .read()
        .ok()
        .map(|config| config.steam_web_api_key.clone());
    let key_ref = api_key.as_deref();

    bridge
        .build_maps_response(key_ref)
        .await
        .map(Json)
        .map_err(|e| {
            eprintln!("Failed to build maps response: {}", e);
            Status::InternalServerError
        })
}

pub fn mount_maps_bridge_routes() -> Vec<Route> {
    routes![sync_registry, registry_updates, list_maps]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;
    use crate::maps_bridge::models::DaemonSourceKind;

    fn test_config() -> Config {
        Config::from_toml_str(
            r#"
frontend_admins = []
[server_daemon]
sync_api_key = "test-secret"
"#,
        )
        .expect("config")
    }

    #[test]
    fn verify_sync_token_accepts_bearer() {
        let dir = tempfile::tempdir().expect("tempdir");
        let config = test_config();
        let bridge = MapsBridgeState::from_config(&config, dir.path().to_path_buf()).expect("bridge");

        assert!(bridge.verify_sync_token(
            &config,
            Some("Bearer test-secret")
        ));
        assert!(!bridge.verify_sync_token(&config, Some("Bearer wrong")));
        assert!(!bridge.verify_sync_token(&config, None));
    }

    #[tokio::test]
    async fn build_maps_response_returns_empty_when_registry_missing() {
        let dir = tempfile::tempdir().expect("tempdir");

        let config = Config::from_toml_str(&format!(
            r#"
frontend_admins = []
[server_daemon]
registry_path = "{}"
daemon_url = ""
"#,
            dir.path().join("maps_registry.json").display()
        ))
        .expect("config");

        let bridge = MapsBridgeState::from_config(&config, dir.path().to_path_buf()).expect("bridge");
        let response = bridge.build_maps_response(None).await.expect("response");

        assert!(response.maps.is_empty());
        assert!(response.stale);
        assert_eq!(response.source, MapsDataSource::RegistryStale);
    }

    #[tokio::test]
    async fn build_maps_response_serves_synced_registry() {
        let dir = tempfile::tempdir().expect("tempdir");
        let registry_path = dir.path().join("maps_registry.json");

        let entry = DaemonMapEntry {
            id: 1,
            name: "Live Map".to_string(),
            source_url: "https://example.com/map.zip".to_string(),
            source_kind: DaemonSourceKind::Other,
            workshop_id: None,
            installed_path: "live.vpk".to_string(),
            installed_at: Utc::now(),
            workshop_updated_at: None,
            version: None,
            checksum: None,
            checksum_kind: None,
        };

        save_registry(&registry_path, vec![entry]).expect("save");

        let config = Config::from_toml_str(&format!(
            r#"
frontend_admins = []
[server_daemon]
registry_path = "{}"
daemon_url = ""
stale_after_secs = 0
"#,
            registry_path.display()
        ))
        .expect("config");

        let bridge = MapsBridgeState::from_config(&config, dir.path().to_path_buf()).expect("bridge");
        let response = bridge.build_maps_response(None).await.expect("response");

        assert_eq!(response.maps.len(), 1);
        assert_eq!(response.maps[0].mapName, "Live Map");
        assert!(!response.stale);
        assert_eq!(response.source, MapsDataSource::Registry);
    }

    #[tokio::test]
    async fn build_maps_response_serves_workshop_preview_from_cache() {
        use workshop_previews::{save_preview_cache, PreviewCacheEntry, WorkshopPreviewCache};

        let dir = tempfile::tempdir().expect("tempdir");
        let registry_path = dir.path().join("maps_registry.json");
        let preview_path = dir.path().join("workshop_previews.json");

        let entry = DaemonMapEntry {
            id: 1,
            name: "2 Evil Eyes".to_string(),
            source_url: String::new(),
            source_kind: DaemonSourceKind::Workshop,
            workshop_id: Some(381419931),
            installed_path: "2evileyes.vpk".to_string(),
            installed_at: Utc::now(),
            workshop_updated_at: None,
            version: None,
            checksum: None,
            checksum_kind: None,
        };

        save_registry(&registry_path, vec![entry]).expect("save");

        let mut cache = WorkshopPreviewCache::default();
        cache.insert(
            381419931,
            PreviewCacheEntry {
                preview_url: "https://images.steamusercontent.com/ugc/test-preview/".to_string(),
                workshop_updated_at: None,
                fetched_at: Utc::now(),
            },
        );
        save_preview_cache(&preview_path, &cache).expect("save previews");

        let config = Config::from_toml_str(&format!(
            r#"
frontend_admins = []
[server_daemon]
registry_path = "{}"
daemon_url = ""
stale_after_secs = 0
"#,
            registry_path.display()
        ))
        .expect("config");

        let bridge = MapsBridgeState::from_config(&config, dir.path().to_path_buf()).expect("bridge");
        let response = bridge.build_maps_response(None).await.expect("response");

        assert_eq!(response.maps.len(), 1);
        assert_eq!(
            response.maps[0].previewUrl.as_deref(),
            Some("https://images.steamusercontent.com/ugc/test-preview/")
        );
    }
}
