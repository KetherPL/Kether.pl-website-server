// SPDX-License-Identifier: GPL-3.0-only

mod admin_install;
mod admin_manage;
mod daemon_client;
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
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, post, routes, Route, State};

use crate::auth::AdminUser;
use crate::steam_bot::registry::ConfigHandle;

use admin_install::{
    proxy_daemon_install_map, proxy_daemon_l4d2center_install, resolve_install_target,
    resolved_mode_label, validate_optional_install_name, AdminInstallMapRequest,
    AdminInstallMapResponse, ResolvedInstallTarget,
};
use admin_manage::{
    proxy_daemon_get_map, proxy_daemon_l4d2center_update, proxy_daemon_uninstall_map,
    proxy_daemon_updates_status, proxy_daemon_workshop_update, MapUpdateOutcome,
};
use daemon_client::with_daemon_auth;

use mapping::daemon_entry_to_website;
use models::{
    AdminApplyUpdatesRequest, AdminApplyUpdatesResponse, AdminMapDetailResponse,
    AdminMapUpdatesStatus, AdminUpdateCheckResponse, DaemonApiResponse, DaemonMapEntry,
    DaemonSourceKind, MapsDataSource, MapsListResponse, SyncRequest, UpdateStatus, UpdatesResponse,
};
use registry_store::{load_registry, resolve_data_path, save_registry};
use workshop_previews::{
    load_preview_cache, preview_cache_path_from_registry, refresh_preview_cache,
    STEAM_PUBLISHED_FILE_DETAILS_URL,
};

pub struct MapsBridgeState {
    pub registry_path: PathBuf,
    pub daemon_url: String,
    pub sync_api_key: Option<String>,
    pub stale_after_secs: u64,
    http_client: Client,
    install_http_client: Client,
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

        let install_http_client = Client::builder()
            .timeout(Duration::from_secs(600))
            .user_agent(format!(
                "{}/{}",
                env!("CARGO_PKG_NAME"),
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .map_err(|e| format!("Failed to build install HTTP client: {}", e))?;

        Ok(Self {
            registry_path: resolve_data_path(
                &base_path,
                &config.server_daemon_registry_path,
                "maps_registry.json",
            ),
            daemon_url: config.server_daemon_url.clone(),
            sync_api_key: (!config.server_daemon_sync_api_key.trim().is_empty())
                .then(|| config.server_daemon_sync_api_key.trim().to_string()),
            stale_after_secs: config.server_daemon_stale_after_secs,
            http_client,
            install_http_client,
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

        let response = with_daemon_auth(
            self.http_client.get(&url),
            self.sync_api_key.as_deref(),
        )
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

    pub async fn refresh_registry_from_daemon(&self, steam_api_key: Option<&str>) -> Result<(), String> {
        let maps = self.fetch_maps_from_daemon().await?;
        save_registry(&self.registry_path, maps.clone())?;
        self.refresh_workshop_previews(&maps, steam_api_key).await;
        Ok(())
    }

    pub async fn admin_install_map(
        &self,
        request: AdminInstallMapRequest,
    ) -> Result<AdminInstallMapResponse, String> {
        if self.daemon_url.trim().is_empty() {
            return Err("Daemon URL not configured".to_string());
        }

        let name = validate_optional_install_name(&request.name)?;
        let target = resolve_install_target(&request.mode, &request.input)
            .map_err(|error| error.to_string())?;

        let map_id = match &target {
            ResolvedInstallTarget::L4d2Center { name: catalog_name } => {
                proxy_daemon_l4d2center_install(
                    &self.install_http_client,
                    &self.daemon_url,
                    self.sync_api_key.as_deref(),
                    catalog_name,
                )
                .await?
            }
            _ => {
                proxy_daemon_install_map(
                    &self.install_http_client,
                    &self.daemon_url,
                    self.sync_api_key.as_deref(),
                    &target,
                    name,
                )
                .await?
            }
        };

        Ok(AdminInstallMapResponse {
            map_id,
            resolved_mode: resolved_mode_label(&target).to_string(),
        })
    }

    pub async fn admin_map_detail(&self, id: u64) -> Result<AdminMapDetailResponse, String> {
        if self.daemon_url.trim().is_empty() {
            return Err("Daemon URL not configured".to_string());
        }
        proxy_daemon_get_map(
            &self.http_client,
            &self.daemon_url,
            self.sync_api_key.as_deref(),
            id,
        )
            .await
            .map(Into::into)
    }

    pub async fn admin_uninstall_map(&self, id: u64) -> Result<(), String> {
        if self.daemon_url.trim().is_empty() {
            return Err("Daemon URL not configured".to_string());
        }
        proxy_daemon_uninstall_map(
            &self.install_http_client,
            &self.daemon_url,
            self.sync_api_key.as_deref(),
            id,
        )
        .await
    }

    pub async fn admin_check_update_map(
        &self,
        id: u64,
    ) -> Result<AdminUpdateCheckResponse, String> {
        self.apply_map_update(id).await
    }

    pub async fn admin_updates_status(&self) -> Result<AdminMapUpdatesStatus, String> {
        if self.daemon_url.trim().is_empty() {
            return Err("Daemon URL not configured".to_string());
        }
        proxy_daemon_updates_status(
            &self.http_client,
            &self.daemon_url,
            self.sync_api_key.as_deref(),
        )
        .await
    }

    pub async fn admin_apply_updates(
        &self,
        map_id: Option<u64>,
    ) -> Result<AdminApplyUpdatesResponse, String> {
        if self.daemon_url.trim().is_empty() {
            return Err("Daemon URL not configured".to_string());
        }

        let ids: Vec<u64> = if let Some(id) = map_id {
            vec![id]
        } else {
            let status = self.admin_updates_status().await?;
            status.available.into_iter().map(|item| item.map_id).collect()
        };

        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            results.push(self.apply_map_update(id).await?);
        }
        Ok(AdminApplyUpdatesResponse { results })
    }

    async fn apply_map_update(&self, id: u64) -> Result<AdminUpdateCheckResponse, String> {
        if self.daemon_url.trim().is_empty() {
            return Err("Daemon URL not configured".to_string());
        }

        let current = proxy_daemon_get_map(
            &self.http_client,
            &self.daemon_url,
            self.sync_api_key.as_deref(),
            id,
        )
        .await?;
        let outcome = match current.source_kind {
            DaemonSourceKind::Workshop => {
                proxy_daemon_workshop_update(
                    &self.install_http_client,
                    &self.daemon_url,
                    self.sync_api_key.as_deref(),
                    id,
                )
                .await?
            }
            DaemonSourceKind::L4d2Center => {
                proxy_daemon_l4d2center_update(
                    &self.install_http_client,
                    &self.daemon_url,
                    self.sync_api_key.as_deref(),
                    id,
                )
                .await?
            }
            DaemonSourceKind::SirPlease | DaemonSourceKind::Other => {
                return Ok(AdminUpdateCheckResponse {
                    status: UpdateStatus::Unsupported,
                    message: "Updates are not supported for this addon source".to_string(),
                    map: Some(current.into()),
                });
            }
        };

        Ok(match outcome {
            MapUpdateOutcome::Updated(entry) => AdminUpdateCheckResponse {
                status: UpdateStatus::Updated,
                message: "Addon was updated successfully".to_string(),
                map: Some(entry.into()),
            },
            MapUpdateOutcome::UpToDate => AdminUpdateCheckResponse {
                status: UpdateStatus::UpToDate,
                message: "Addon is already up to date".to_string(),
                map: Some(current.into()),
            },
            MapUpdateOutcome::Failed(error) => AdminUpdateCheckResponse {
                status: UpdateStatus::Failed,
                message: error,
                map: Some(current.into()),
            },
        })
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminInstallErrorResponse {
    pub error: String,
}

#[post("/maps/admin/install", data = "<body>")]
pub async fn admin_install_map(
    _admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    config_handle: &State<ConfigHandle>,
    body: Json<AdminInstallMapRequest>,
) -> Result<Json<AdminInstallMapResponse>, (Status, Json<AdminInstallErrorResponse>)> {
    let api_key = config_handle
        .read()
        .ok()
        .map(|config| config.steam_web_api_key.clone());
    let key_ref = api_key.as_deref();

    let response = bridge
        .admin_install_map(body.into_inner())
        .await
        .map_err(|error| {
            eprintln!("Admin map install failed: {}", error);
            let status = if error.contains("not configured")
                || error.contains("Daemon request failed")
                || error.contains("Daemon returned HTTP")
            {
                Status::BadGateway
            } else if error.contains("required")
                || error.contains("Invalid")
                || error.contains("Expected")
                || error.contains("too long")
                || error.contains("Could not detect")
            {
                Status::BadRequest
            } else {
                Status::InternalServerError
            };
            (status, Json(AdminInstallErrorResponse { error }))
        })?;

    if let Err(error) = bridge.refresh_registry_from_daemon(key_ref).await {
        eprintln!("Registry refresh after install failed (non-fatal): {}", error);
    }

    Ok(Json(response))
}

fn admin_proxy_error(error: String) -> (Status, Json<AdminInstallErrorResponse>) {
    let status = if error.to_lowercase().contains("not found") {
        Status::NotFound
    } else if error.contains("not configured")
        || error.contains("Daemon request failed")
        || error.contains("Daemon returned HTTP")
    {
        Status::BadGateway
    } else {
        Status::InternalServerError
    };
    (status, Json(AdminInstallErrorResponse { error }))
}

fn steam_api_key(config_handle: &State<ConfigHandle>) -> Option<String> {
    config_handle
        .read()
        .ok()
        .map(|config| config.steam_web_api_key.clone())
}

#[get("/maps/admin/<id>")]
pub async fn get_admin_map_detail(
    _admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    id: u64,
) -> Result<Json<AdminMapDetailResponse>, (Status, Json<AdminInstallErrorResponse>)> {
    bridge
        .admin_map_detail(id)
        .await
        .map(Json)
        .map_err(admin_proxy_error)
}

#[post("/maps/admin/<id>/uninstall")]
pub async fn admin_uninstall_map(
    _admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    config_handle: &State<ConfigHandle>,
    id: u64,
) -> Result<Status, (Status, Json<AdminInstallErrorResponse>)> {
    bridge
        .admin_uninstall_map(id)
        .await
        .map_err(admin_proxy_error)?;

    let api_key = steam_api_key(config_handle);
    if let Err(error) = bridge.refresh_registry_from_daemon(api_key.as_deref()).await {
        eprintln!("Registry refresh after uninstall failed (non-fatal): {}", error);
    }
    Ok(Status::Ok)
}

#[post("/maps/admin/<id>/check-update")]
pub async fn admin_check_update_map(
    _admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    config_handle: &State<ConfigHandle>,
    id: u64,
) -> Result<Json<AdminUpdateCheckResponse>, (Status, Json<AdminInstallErrorResponse>)> {
    let response = bridge
        .admin_check_update_map(id)
        .await
        .map_err(admin_proxy_error)?;

    if response.status == UpdateStatus::Updated {
        let api_key = steam_api_key(config_handle);
        if let Err(error) = bridge.refresh_registry_from_daemon(api_key.as_deref()).await {
            eprintln!("Registry refresh after map update failed (non-fatal): {}", error);
        }
    }

    Ok(Json(response))
}

#[get("/maps/admin/updates")]
pub async fn admin_list_updates(
    _admin: AdminUser,
    bridge: &State<MapsBridgeState>,
) -> Result<Json<AdminMapUpdatesStatus>, (Status, Json<AdminInstallErrorResponse>)> {
    bridge
        .admin_updates_status()
        .await
        .map(Json)
        .map_err(admin_proxy_error)
}

#[post("/maps/admin/updates/apply", data = "<body>")]
pub async fn admin_apply_updates(
    _admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    config_handle: &State<ConfigHandle>,
    body: Json<AdminApplyUpdatesRequest>,
) -> Result<Json<AdminApplyUpdatesResponse>, (Status, Json<AdminInstallErrorResponse>)> {
    let response = bridge
        .admin_apply_updates(body.map_id)
        .await
        .map_err(admin_proxy_error)?;

    if response
        .results
        .iter()
        .any(|result| result.status == UpdateStatus::Updated)
    {
        let api_key = steam_api_key(config_handle);
        if let Err(error) = bridge.refresh_registry_from_daemon(api_key.as_deref()).await {
            eprintln!("Registry refresh after map updates failed (non-fatal): {}", error);
        }
    }

    Ok(Json(response))
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
    routes![
        sync_registry,
        registry_updates,
        admin_install_map,
        admin_list_updates,
        admin_apply_updates,
        get_admin_map_detail,
        admin_uninstall_map,
        admin_check_update_map,
        list_maps
    ]
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

        assert_eq!(bridge.sync_api_key.as_deref(), Some("test-secret"));
        assert!(bridge.verify_sync_token(
            &config,
            Some("Bearer test-secret")
        ));
        assert!(!bridge.verify_sync_token(&config, Some("Bearer wrong")));
        assert!(!bridge.verify_sync_token(&config, None));
    }

    #[test]
    fn mount_registers_admin_manage_routes() {
        let paths: Vec<String> = mount_maps_bridge_routes()
            .iter()
            .map(|route| route.uri.to_string())
            .collect();

        assert!(paths.iter().any(|path| path == "/maps/admin/<id>"));
        assert!(paths
            .iter()
            .any(|path| path == "/maps/admin/<id>/uninstall"));
        assert!(paths
            .iter()
            .any(|path| path == "/maps/admin/<id>/check-update"));
        assert!(paths.iter().any(|path| path == "/maps/admin/updates"));
        assert!(paths
            .iter()
            .any(|path| path == "/maps/admin/updates/apply"));
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
