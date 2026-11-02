// SPDX-License-Identifier: GPL-3.0-only

mod admin_install;
mod admin_manage;
mod daemon_client;
mod l4d2center_index;
mod mapping;
mod models;
mod registry_store;
mod suggestion_conflicts;
mod suggestions;
mod suggestions_store;
mod workshop_previews;

use std::path::{Path, PathBuf};
use std::time::Duration;

use chrono::Utc;
use reqwest::Client;
use rocket::http::Status;
use rocket::request::{FromRequest, Outcome, Request};
use rocket::serde::json::Json;
use rocket::serde::{Deserialize, Serialize};
use rocket::{get, options, post, routes, Route, State};

use crate::auth::AdminUser;
use crate::admin_actions::{config_snapshot as audit_config_snapshot, ActionChoice, ActionOutcome, AdminAction, AdminActionLog, RecordedChange};
use rocket::serde::json::{json, Value};
use crate::steam_bot::registry::ConfigHandle;

use admin_install::{
    proxy_daemon_install_map, proxy_daemon_l4d2center_install, resolve_install_target,
    resolved_mode_label, validate_optional_install_name, AdminInstallMapRequest,
    AdminInstallMapResponse, ResolvedInstallTarget,
};
use admin_manage::{
    proxy_daemon_get_map, proxy_daemon_l4d2center_update, proxy_daemon_uninstall_map,
    proxy_daemon_updates_status, proxy_daemon_updates_check, proxy_daemon_workshop_update, MapUpdateOutcome,
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
    WorkshopPreviewCache, STEAM_PUBLISHED_FILE_DETAILS_URL,
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

    async fn audit_map_data(&self, logging: bool, id: u64) -> Result<Value, String> {
        if !logging { return Ok(json!({"id": id})); }
        let before = proxy_daemon_get_map(&self.http_client, &self.daemon_url,
            self.sync_api_key.as_deref(), id).await?;
        Ok(json!({"id": id, "name": &before.name, "url": &before.source_url, "before": &before}))
    }

    fn audit_installed_map_data(&self, id: u64) -> Value {
        let installed = load_registry(&self.registry_path).ok()
            .and_then(|registry| registry.maps.into_iter().find(|map| map.id == id));
        match installed {
            Some(map) => json!({"id": id, "name": &map.name, "url": &map.source_url, "after": &map}),
            None => json!({"id": id}),
        }
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

    pub async fn admin_updates_check(&self) -> Result<AdminMapUpdatesStatus, String> {
        if self.daemon_url.trim().is_empty() {
            return Err("Daemon URL not configured".to_string());
        }
        proxy_daemon_updates_check(
            &self.install_http_client,
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

        let status = self.admin_updates_status().await?;
        let in_progress_ids: std::collections::HashSet<u64> =
            status.in_progress.iter().map(|item| item.map_id).collect();

        let ids: Vec<u64> = if let Some(id) = map_id {
            if in_progress_ids.contains(&id) {
                return Ok(AdminApplyUpdatesResponse {
                    results: vec![AdminUpdateCheckResponse {
                        status: UpdateStatus::Failed,
                        message: "Map update is already in progress".to_string(),
                        map: None,
                    }],
                });
            }
            vec![id]
        } else {
            status
                .available
                .into_iter()
                .map(|item| item.map_id)
                .filter(|id| !in_progress_ids.contains(id))
                .collect()
        };

        let mut results = Vec::with_capacity(ids.len());
        for id in ids {
            match self.apply_map_update(id).await {
                Ok(result) => results.push(result),
                Err(error) => results.push(AdminUpdateCheckResponse {
                    status: UpdateStatus::Failed,
                    message: error,
                    map: None,
                }),
            }
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

/// Minimal, chat-bot-facing representation of an installed map link lookup result.
pub struct InstalledMapLink {
    pub map_name: String,
    pub source: String,
    pub url: Option<String>,
}

/// Looks up an installed map in the local registry cache by name (case-insensitive exact
/// match, falling back to substring match). Synchronous, local-only read — no daemon
/// round-trip — used by the Steam chat bot's `!plan ... l m <name>` link lookup.
pub fn find_installed_map_link(
    base_path: &Path,
    config: &crate::config::Config,
    query: &str,
) -> Option<InstalledMapLink> {
    let entry = find_installed_registry_entry(base_path, config, query)?;
    let website_entry = daemon_entry_to_website(&entry, &WorkshopPreviewCache::default());
    Some(InstalledMapLink {
        map_name: website_entry.mapName,
        source: website_entry.source,
        url: website_entry.downloadUrl,
    })
}

/// Chat-bot-facing details of a single installed map (`!mapinfo`).
pub struct InstalledMapInfo {
    pub map_name: String,
    pub source: String,
    pub checksum: Option<String>,
    pub checksum_kind: Option<String>,
    pub workshop_updated_at: Option<chrono::DateTime<Utc>>,
    pub installed_at: chrono::DateTime<Utc>,
    pub workshop_id: Option<u64>,
    /// Source/download URL (source URL, or the workshop page when only an ID is known).
    pub url: Option<String>,
    /// Size on disk, as last reported by the daemon.
    pub size_bytes: Option<u64>,
}

/// Looks up an installed map by name (same matching as [`find_installed_map_link`]) and returns
/// its details. Local registry read only — no daemon round-trip.
pub fn find_installed_map_info(
    base_path: &Path,
    config: &crate::config::Config,
    query: &str,
) -> Option<InstalledMapInfo> {
    let entry = find_installed_registry_entry(base_path, config, query)?;
    let website_entry = daemon_entry_to_website(&entry, &WorkshopPreviewCache::default());
    Some(InstalledMapInfo {
        map_name: entry.name,
        source: website_entry.source,
        checksum: entry.checksum,
        checksum_kind: entry.checksum_kind,
        workshop_updated_at: entry.workshop_updated_at,
        installed_at: entry.installed_at,
        workshop_id: entry.workshop_id,
        url: website_entry.downloadUrl,
        size_bytes: entry.size_bytes,
    })
}

/// Finds an installed map in the local registry cache by name: case-insensitive exact match,
/// falling back to the first case-insensitive substring match.
fn find_installed_registry_entry(
    base_path: &Path,
    config: &crate::config::Config,
    query: &str,
) -> Option<DaemonMapEntry> {
    let query = query.trim();
    if query.is_empty() {
        return None;
    }
    let registry_path =
        resolve_data_path(base_path, &config.server_daemon_registry_path, "maps_registry.json");
    let registry = load_registry(&registry_path).ok()?;
    let query_lower = query.to_lowercase();

    let index = registry
        .maps
        .iter()
        .position(|m| m.name.eq_ignore_ascii_case(query))
        .or_else(|| {
            registry
                .maps
                .iter()
                .position(|m| m.name.to_lowercase().contains(&query_lower))
        })?;
    registry.maps.into_iter().nth(index)
}

/// Chat-bot-facing snapshot of installed maps (with their source labels) and pending map
/// suggestions. Built from local files only — no daemon round-trip.
pub struct ChatMapsListing {
    /// `(source label, map name)` pairs, where the label matches the website's source names
    /// ("L4D2Center", "SirPlease", "Workshop", "Other").
    pub installed: Vec<(String, String)>,
    /// Titles of pending map suggestions.
    pub suggestions: Vec<String>,
}

/// Reads the local maps registry cache and the map suggestions store for the Steam chat bot's
/// `!maps` command. A missing/unreadable suggestions store is treated as empty.
pub fn list_maps_for_chat(
    base_path: &Path,
    config: &crate::config::Config,
) -> Result<ChatMapsListing, String> {
    let registry_path =
        resolve_data_path(base_path, &config.server_daemon_registry_path, "maps_registry.json");
    let registry = load_registry(&registry_path)?;

    let installed = registry
        .maps
        .iter()
        .map(|entry| {
            (
                models::daemon_source_to_website(entry.source_kind).to_string(),
                entry.name.clone(),
            )
        })
        .collect();

    let suggestions = suggestions_store::SuggestionsStore::new(
        suggestions_store::SuggestionsStore::path_from_registry(&registry_path),
    )
    .list()
    .map(|items| items.into_iter().map(|s| s.title).collect())
    .unwrap_or_else(|e| {
        eprintln!("!maps: failed to load map suggestions (treating as empty): {}", e);
        Vec::new()
    });

    Ok(ChatMapsListing { installed, suggestions })
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

fn audit_map_error((status, error): (Status, String)) -> (Status, Json<AdminInstallErrorResponse>) {
    (status, Json(AdminInstallErrorResponse { error }))
}

fn update_audit_outcome(results: &[AdminUpdateCheckResponse]) -> (ActionOutcome, Option<String>) {
    let failures: Vec<_> = results
        .iter()
        .filter(|result| result.status == UpdateStatus::Failed)
        .map(|result| result.message.clone())
        .collect();
    if !failures.is_empty() {
        (ActionOutcome::Failed, Some(failures.join("; ")))
    } else if results
        .iter()
        .any(|result| result.status == UpdateStatus::Updated)
    {
        (ActionOutcome::Completed, None)
    } else {
        (ActionOutcome::Unchanged, None)
    }
}

#[post("/maps/admin/install", data = "<body>")]
pub async fn admin_install_map(
    admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    log: &State<AdminActionLog>,
    config_handle: &State<ConfigHandle>,
    body: Json<AdminInstallMapRequest>,
) -> Result<Json<AdminInstallMapResponse>, (Status, Json<AdminInstallErrorResponse>)> {
    let config = audit_config_snapshot(config_handle);
    let data = json!({"submitted": &body.0, "name": &body.name, "url": &body.input});
    log.record_change(
        &config,
        admin.0,
        AdminAction::Map,
        ActionChoice::Install,
        data,
        async {
            let response = async {
                let api_key = config_handle
                    .read()
                    .ok()
                    .map(|config| config.steam_web_api_key.clone());
                let key_ref = api_key.as_deref();

                let response =
                    bridge
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
                    eprintln!(
                        "Registry refresh after install failed (non-fatal): {}",
                        error
                    );
                }

                Ok(Json(response))
            }
            .await
            .map_err(
                |(status, error): (Status, Json<AdminInstallErrorResponse>)| {
                    (status, error.error.clone())
                },
            )?;
            let mut final_data = bridge.audit_installed_map_data(response.map_id);
            final_data["result"] = json!(&response.0);
            Ok(RecordedChange::completed(response, final_data))
        },
    )
    .await
    .map_err(audit_map_error)
}

fn admin_proxy_error(error: String) -> (Status, Json<AdminInstallErrorResponse>) {
    let lower = error.to_lowercase();
    let status = if lower.contains("already in progress") {
        Status::Conflict
    } else if lower.contains("not found") {
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
    admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    log: &State<AdminActionLog>,
    config_handle: &State<ConfigHandle>,
    id: u64,
) -> Result<Status, (Status, Json<AdminInstallErrorResponse>)> {
    let config = audit_config_snapshot(config_handle);
    let data = bridge
        .audit_map_data(config.admin_action_logging, id)
        .await
        .map_err(admin_proxy_error)?;
    log.record_change(
        &config,
        admin.0,
        AdminAction::Map,
        ActionChoice::Delete,
        data,
        async {
            let response = async {
                bridge
                    .admin_uninstall_map(id)
                    .await
                    .map_err(admin_proxy_error)?;

                let api_key = steam_api_key(config_handle);
                if let Err(error) = bridge
                    .refresh_registry_from_daemon(api_key.as_deref())
                    .await
                {
                    eprintln!(
                        "Registry refresh after uninstall failed (non-fatal): {}",
                        error
                    );
                }
                Ok(Status::Ok)
            }
            .await
            .map_err(
                |(status, error): (Status, Json<AdminInstallErrorResponse>)| {
                    (status, error.error.clone())
                },
            )?;
            Ok(RecordedChange::completed(response, json!({"after": null})))
        },
    )
    .await
    .map_err(audit_map_error)
}

#[post("/maps/admin/<id>/check-update")]
pub async fn admin_check_update_map(
    admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    log: &State<AdminActionLog>,
    config_handle: &State<ConfigHandle>,
    id: u64,
) -> Result<Json<AdminUpdateCheckResponse>, (Status, Json<AdminInstallErrorResponse>)> {
    let config = audit_config_snapshot(config_handle);
    let data = bridge
        .audit_map_data(config.admin_action_logging, id)
        .await
        .map_err(admin_proxy_error)?;
    log.record_change(
        &config,
        admin.0,
        AdminAction::Map,
        ActionChoice::Update,
        data,
        async {
            let response = async {
                let response = bridge
                    .admin_check_update_map(id)
                    .await
                    .map_err(admin_proxy_error)?;

                if response.status == UpdateStatus::Updated {
                    let api_key = steam_api_key(config_handle);
                    if let Err(error) = bridge
                        .refresh_registry_from_daemon(api_key.as_deref())
                        .await
                    {
                        eprintln!(
                            "Registry refresh after map update failed (non-fatal): {}",
                            error
                        );
                    }
                }

                Ok(Json(response))
            }
            .await
            .map_err(
                |(status, error): (Status, Json<AdminInstallErrorResponse>)| {
                    (status, error.error.clone())
                },
            )?;
            let (outcome, error) = update_audit_outcome(std::slice::from_ref(&response.0));
            Ok(RecordedChange {
                data: json!({"after": &response.map, "result": &response.0}),
                value: response,
                outcome,
                error,
            })
        },
    )
    .await
    .map_err(audit_map_error)
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

#[post("/maps/admin/updates/check")]
pub async fn admin_check_updates(
    _admin: AdminUser,
    bridge: &State<MapsBridgeState>,
) -> Result<Json<AdminMapUpdatesStatus>, (Status, Json<AdminInstallErrorResponse>)> {
    bridge
        .admin_updates_check()
        .await
        .map(Json)
        .map_err(admin_proxy_error)
}

#[post("/maps/admin/updates/apply", data = "<body>")]
pub async fn admin_apply_updates(
    admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    log: &State<AdminActionLog>,
    config_handle: &State<ConfigHandle>,
    body: Json<AdminApplyUpdatesRequest>,
) -> Result<Json<AdminApplyUpdatesResponse>, (Status, Json<AdminInstallErrorResponse>)> {
    let config = audit_config_snapshot(config_handle);
    let choice = if body.map_id.is_some() {
        ActionChoice::Update
    } else {
        ActionChoice::UpdateAll
    };
    let data = if let Some(id) = body.map_id {
        bridge
            .audit_map_data(config.admin_action_logging, id)
            .await
            .map_err(admin_proxy_error)?
    } else if config.admin_action_logging {
        let before = bridge
            .fetch_maps_from_daemon()
            .await
            .map_err(admin_proxy_error)?;
        json!({"before": before})
    } else {
        json!({})
    };
    log.record_change(&config, admin.0, AdminAction::Map, choice, data, async {
        let response = async {
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
                if let Err(error) = bridge
                    .refresh_registry_from_daemon(api_key.as_deref())
                    .await
                {
                    eprintln!(
                        "Registry refresh after map updates failed (non-fatal): {}",
                        error
                    );
                }
            }

            Ok(Json(response))
        }
        .await
        .map_err(
            |(status, error): (Status, Json<AdminInstallErrorResponse>)| {
                (status, error.error.clone())
            },
        )?;
        let (outcome, error) = update_audit_outcome(&response.results);
        Ok(RecordedChange {
            data: json!({"result": &response.0}),
            value: response,
            outcome,
            error,
        })
    })
    .await
    .map_err(audit_map_error)
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

#[options("/maps")]
pub fn options_list_maps() -> Status {
    Status::Ok
}

#[options("/maps/admin/install")]
pub fn options_admin_install_map() -> Status {
    Status::Ok
}

#[options("/maps/admin/<id>")]
pub fn options_admin_map_detail(id: u64) -> Status {
    let _ = id;
    Status::Ok
}

#[options("/maps/admin/<id>/uninstall")]
pub fn options_admin_uninstall_map(id: u64) -> Status {
    let _ = id;
    Status::Ok
}

#[options("/maps/admin/<id>/check-update")]
pub fn options_admin_check_update_map(id: u64) -> Status {
    let _ = id;
    Status::Ok
}

#[options("/maps/admin/updates")]
pub fn options_admin_list_updates() -> Status {
    Status::Ok
}

#[options("/maps/admin/updates/check")]
pub fn options_admin_check_updates() -> Status {
    Status::Ok
}

#[options("/maps/admin/updates/apply")]
pub fn options_admin_apply_updates() -> Status {
    Status::Ok
}

pub fn mount_maps_bridge_routes() -> Vec<Route> {
    routes![
        sync_registry,
        registry_updates,
        admin_install_map,
        admin_list_updates,
        admin_check_updates,
        admin_apply_updates,
        get_admin_map_detail,
        admin_uninstall_map,
        admin_check_update_map,
        list_maps,
        suggestions::list_map_suggestions,
        suggestions::create_map_suggestion,
        suggestions::deny_map_suggestion,
        suggestions::accept_map_suggestion,
        options_list_maps,
        options_admin_install_map,
        options_admin_map_detail,
        options_admin_uninstall_map,
        options_admin_check_update_map,
        options_admin_list_updates,
        options_admin_check_updates,
        options_admin_apply_updates,
        suggestions::options_map_suggestions,
        suggestions::options_deny_map_suggestion,
        suggestions::options_accept_map_suggestion,
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

    #[tokio::test]
    async fn map_audit_deletion_saves_name_link_and_previous_version_before_daemon_mutation() {
        use tokio::io::{AsyncReadExt, AsyncWriteExt};
        for failed in [false, true] {
            let directory = tempfile::tempdir().expect("directory");
            let log_path = directory.path().join("log_admin_actions.json");
            let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.expect("listener");
            let mut config = test_config();
            config.server_daemon_url = format!("http://{}", listener.local_addr().expect("address"));
            let mut before = map_entry(42, "No Mercy Rehab", Some(123), "https://steamcommunity.com/sharedfiles/filedetails/?id=123");
            before.version = Some("1.0".to_string());
            let server_log_path = log_path.clone();
            let daemon = tokio::spawn(async move {
                let responses = if failed {
                    vec![(200, json!({"success": true, "data": before})), (500, json!({"success": false, "error": "Uninstall failed"}))]
                } else {
                    vec![(200, json!({"success": true, "data": before})), (200, json!({"success": true, "data": null})), (200, json!({"success": true, "data": []}))]
                };
                for (index, (status, body)) in responses.into_iter().enumerate() {
                    let (mut stream, _) = listener.accept().await.expect("accept");
                    let mut request = Vec::new();
                    let mut buffer = [0; 1024];
                    while !request.windows(4).any(|window| window == b"\r\n\r\n") {
                        let count = stream.read(&mut buffer).await.expect("read");
                        assert!(count > 0);
                        request.extend_from_slice(&buffer[..count]);
                    }
                    if index == 1 {
                        assert!(String::from_utf8_lossy(&request).starts_with("POST /api/maps/uninstall/42 "));
                        let saved: Value = rocket::serde::json::from_slice(&std::fs::read(&server_log_path).expect("durable log before mutation")).expect("json");
                        assert_eq!(saved[0]["outcome"], "requested");
                        assert_eq!(saved[0]["additional_data"]["before"]["version"], "1.0");
                    }
                    let body = body.to_string();
                    let response = format!("HTTP/1.1 {status} Test\r\nContent-Type: application/json\r\nContent-Length: {}\r\nConnection: close\r\n\r\n{body}", body.len());
                    stream.write_all(response.as_bytes()).await.expect("write");
                }
            });
            let bridge = MapsBridgeState::from_config(&config, directory.path().to_path_buf()).expect("bridge");
            let handle: ConfigHandle = std::sync::Arc::new(std::sync::RwLock::new(std::sync::Arc::new(config)));
            let log = AdminActionLog::new(log_path);
            let result = admin_uninstall_map(AdminUser(76561198000000001), State::from(&bridge), State::from(&log), State::from(&handle), 42).await;
            assert_eq!(result.is_err(), failed);
            daemon.await.expect("daemon");
            let entries = log.history(15).await.expect("history");
            assert_eq!(entries[0].action_id, 4);
            assert_eq!(entries[0].outcome, if failed { ActionOutcome::Failed } else { ActionOutcome::Completed });
            let data = entries[0].additional_data.as_ref().expect("data");
            assert_eq!(data["name"], "No Mercy Rehab");
            assert_eq!(data["url"], "https://steamcommunity.com/sharedfiles/filedetails/?id=123");
            assert_eq!(data["before"]["version"], "1.0");
        }
    }

    #[test]
    fn map_audit_outcomes_distinguish_real_updates_no_changes_and_partial_failure() {
        let result = |status| AdminUpdateCheckResponse { status, message: "result".to_string(), map: None };
        assert_eq!(update_audit_outcome(&[result(UpdateStatus::UpToDate)]).0, ActionOutcome::Unchanged);
        assert_eq!(update_audit_outcome(&[result(UpdateStatus::Updated)]).0, ActionOutcome::Completed);
        assert_eq!(update_audit_outcome(&[result(UpdateStatus::Updated), result(UpdateStatus::Failed)]).0, ActionOutcome::Failed);
    }

    #[tokio::test]
    async fn map_audit_suggestion_denial_retains_full_proposal() {
        let directory = tempfile::tempdir().expect("directory");
        let config = test_config();
        let bridge = MapsBridgeState::from_config(&config, directory.path().to_path_buf()).expect("bridge");
        let proposal = bridge.suggestions_store().create(models::MapSuggestion {
            id: 0, source_kind: models::MapSuggestionSourceKind::Workshop, workshop_id: Some(123),
            l4d2center_name: None, title: "No Mercy Rehab".to_string(), preview_url: None,
            download_link: Some("https://steamcommunity.com/sharedfiles/filedetails/?id=123".to_string()),
            size_bytes: None, proposed_by: "76561198000000002".to_string(), proposed_at: Utc::now(),
        }).expect("proposal");
        let handle: ConfigHandle = std::sync::Arc::new(std::sync::RwLock::new(std::sync::Arc::new(config)));
        let log = AdminActionLog::new(directory.path().join("log_admin_actions.json"));
        let result = suggestions::deny_map_suggestion(AdminUser(76561198000000001), State::from(&bridge), State::from(&log), State::from(&handle), proposal.id).await;
        assert!(result.is_ok());
        let entries = log.history(15).await.expect("history");
        assert_eq!(entries[0].action_id, 8);
        assert_eq!(entries[0].choice, ActionChoice::Deny);
        assert_eq!(entries[0].additional_data.as_ref().expect("snapshot")["name"], "No Mercy Rehab");
        assert_eq!(entries[0].additional_data.as_ref().expect("snapshot")["before"]["proposed_by"], "76561198000000002");
        assert!(bridge.suggestions_store().get(proposal.id).expect("store").is_none());
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
        assert!(paths
            .iter()
            .any(|path| path == "/maps/admin/updates/check"));
        assert!(paths.iter().any(|path| path == "/maps/suggestions"));
        assert!(paths
            .iter()
            .any(|path| path == "/maps/suggestions/<id>/deny"));
        assert!(paths
            .iter()
            .any(|path| path == "/maps/suggestions/<id>/accept"));
        // OPTIONS preflight handlers for browser CORS
        assert_eq!(
            paths
                .iter()
                .filter(|path| path.as_str() == "/maps/admin/updates")
                .count(),
            2,
            "GET + OPTIONS for /maps/admin/updates"
        );
        assert_eq!(
            paths
                .iter()
                .filter(|path| path.as_str() == "/maps/admin/updates/check")
                .count(),
            2,
            "POST + OPTIONS for /maps/admin/updates/check"
        );
    }

    #[test]
    fn admin_proxy_error_maps_already_in_progress_to_conflict() {
        let (status, body) =
            admin_proxy_error("A map update check is already in progress".to_string());
        assert_eq!(status, Status::Conflict);
        assert!(body.error.contains("already in progress"));

        let (status, _) = admin_proxy_error("Map #1 not found".to_string());
        assert_eq!(status, Status::NotFound);
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
            size_bytes: None,
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
            size_bytes: None,
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

    fn map_entry(id: u64, name: &str, workshop_id: Option<u64>, source_url: &str) -> DaemonMapEntry {
        DaemonMapEntry {
            id,
            name: name.to_string(),
            source_url: source_url.to_string(),
            source_kind: if workshop_id.is_some() {
                DaemonSourceKind::Workshop
            } else {
                DaemonSourceKind::Other
            },
            workshop_id,
            installed_path: format!("{}.vpk", name.to_lowercase().replace(' ', "_")),
            installed_at: Utc::now(),
            workshop_updated_at: None,
            version: None,
            checksum: None,
            checksum_kind: None,
            size_bytes: None,
        }
    }

    fn config_with_registry(registry_path: &std::path::Path) -> Config {
        Config::from_toml_str(&format!(
            r#"
frontend_admins = []
[server_daemon]
registry_path = "{}"
daemon_url = ""
"#,
            registry_path.display()
        ))
        .expect("config")
    }

    #[test]
    fn find_installed_map_link_exact_match_returns_workshop_url() {
        let dir = tempfile::tempdir().expect("tempdir");
        let registry_path = dir.path().join("maps_registry.json");
        save_registry(
            &registry_path,
            vec![map_entry(1, "No Mercy", Some(125499071), "")],
        )
        .expect("save");
        let config = config_with_registry(&registry_path);

        let found = find_installed_map_link(dir.path(), &config, "No Mercy").expect("found");
        assert_eq!(found.map_name, "No Mercy");
        assert!(found.url.unwrap().contains("125499071"));
    }

    #[test]
    fn find_installed_map_link_is_case_insensitive_exact_match() {
        let dir = tempfile::tempdir().expect("tempdir");
        let registry_path = dir.path().join("maps_registry.json");
        save_registry(
            &registry_path,
            vec![map_entry(1, "No Mercy", Some(125499071), "")],
        )
        .expect("save");
        let config = config_with_registry(&registry_path);

        let found = find_installed_map_link(dir.path(), &config, "no mercy").expect("found");
        assert_eq!(found.map_name, "No Mercy");
    }

    #[test]
    fn find_installed_map_link_falls_back_to_substring_match() {
        let dir = tempfile::tempdir().expect("tempdir");
        let registry_path = dir.path().join("maps_registry.json");
        save_registry(
            &registry_path,
            vec![map_entry(
                1,
                "Custom Center Map",
                None,
                "https://example.com/center.vpk",
            )],
        )
        .expect("save");
        let config = config_with_registry(&registry_path);

        let found = find_installed_map_link(dir.path(), &config, "center").expect("found");
        assert_eq!(found.map_name, "Custom Center Map");
        assert_eq!(found.url.as_deref(), Some("https://example.com/center.vpk"));
    }

    #[test]
    fn find_installed_map_link_returns_none_when_no_match() {
        let dir = tempfile::tempdir().expect("tempdir");
        let registry_path = dir.path().join("maps_registry.json");
        save_registry(
            &registry_path,
            vec![map_entry(1, "No Mercy", Some(125499071), "")],
        )
        .expect("save");
        let config = config_with_registry(&registry_path);

        assert!(find_installed_map_link(dir.path(), &config, "Dead Center").is_none());
    }

    #[test]
    fn find_installed_map_link_returns_none_for_empty_query() {
        let dir = tempfile::tempdir().expect("tempdir");
        let registry_path = dir.path().join("maps_registry.json");
        save_registry(
            &registry_path,
            vec![map_entry(1, "No Mercy", Some(125499071), "")],
        )
        .expect("save");
        let config = config_with_registry(&registry_path);

        assert!(find_installed_map_link(dir.path(), &config, "   ").is_none());
    }
}
