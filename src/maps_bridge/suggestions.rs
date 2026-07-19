// SPDX-License-Identifier: GPL-3.0-only

use chrono::Utc;
use rocket::http::Status;
use rocket::serde::json::Json;
use rocket::{get, options, post, State};

use crate::auth::{AdminUser, AuthUser};
use crate::steam_bot::registry::ConfigHandle;

use super::admin_install::{
    proxy_daemon_install_map, proxy_daemon_l4d2center_install, resolve_install_target,
    ResolvedInstallTarget,
};
use super::l4d2center_index::{fetch_l4d2center_index, find_l4d2center_entry};
use super::models::{
    workshop_download_url, AcceptMapSuggestionResponse, CreateMapSuggestionRequest, MapSuggestion,
    MapSuggestionSourceKind, MapSuggestionView,
};
use super::AdminInstallErrorResponse;
use super::registry_store::load_registry;
use super::suggestion_conflicts::{check_installed_conflict, conflict_to_view, ConflictCheck};
use super::workshop_previews::fetch_workshop_meta;
use super::MapsBridgeState;

fn suggestion_error(status: Status, error: impl Into<String>) -> (Status, Json<AdminInstallErrorResponse>) {
    (
        status,
        Json(AdminInstallErrorResponse {
            error: error.into(),
        }),
    )
}

fn steam_api_key(config_handle: &State<ConfigHandle>) -> Option<String> {
    config_handle
        .read()
        .ok()
        .map(|config| config.steam_web_api_key.clone())
}

impl MapsBridgeState {
    pub fn suggestions_path(&self) -> std::path::PathBuf {
        super::suggestions_store::SuggestionsStore::path_from_registry(&self.registry_path)
    }

    pub fn suggestions_store(&self) -> super::suggestions_store::SuggestionsStore {
        super::suggestions_store::SuggestionsStore::new(self.suggestions_path())
    }

    fn load_installed_maps(&self) -> Result<Vec<super::models::DaemonMapEntry>, String> {
        Ok(load_registry(&self.registry_path)?.maps)
    }

    pub async fn list_map_suggestions(&self) -> Result<Vec<MapSuggestionView>, String> {
        let suggestions = self.suggestions_store().list()?;
        let maps = self.load_installed_maps().unwrap_or_default();
        Ok(suggestions
            .into_iter()
            .map(|suggestion| {
                let conflict =
                    conflict_to_view(&check_installed_conflict(&suggestion, &maps));
                MapSuggestionView {
                    suggestion,
                    conflict,
                }
            })
            .collect())
    }

    pub async fn create_map_suggestion(
        &self,
        mode: &str,
        input: &str,
        proposed_by: String,
        api_key: Option<&str>,
    ) -> Result<MapSuggestionView, (Status, String)> {
        let target = resolve_install_target(mode, input).map_err(|e| {
            (
                Status::BadRequest,
                e.to_string(),
            )
        })?;

        let suggestion = match target {
            ResolvedInstallTarget::Workshop { workshop_id } => {
                let meta = fetch_workshop_meta(
                    &self.http_client,
                    &self.published_file_details_url,
                    workshop_id,
                    api_key,
                )
                .await
                .map_err(|e| (Status::BadGateway, e))?;
                MapSuggestion {
                    id: 0,
                    source_kind: MapSuggestionSourceKind::Workshop,
                    workshop_id: Some(workshop_id),
                    l4d2center_name: None,
                    title: meta.title,
                    preview_url: meta.preview_url,
                    download_link: Some(workshop_download_url(workshop_id)),
                    size_bytes: meta.size_bytes,
                    proposed_by,
                    proposed_at: Utc::now(),
                }
            }
            ResolvedInstallTarget::L4d2Center { name } => {
                let index = fetch_l4d2center_index(&self.http_client)
                    .await
                    .map_err(|e| (Status::BadGateway, e))?;
                let entry = find_l4d2center_entry(&index, &name).ok_or_else(|| {
                    (
                        Status::NotFound,
                        format!("L4D2Center map '{name}' not found in catalog"),
                    )
                })?;
                MapSuggestion {
                    id: 0,
                    source_kind: MapSuggestionSourceKind::L4d2Center,
                    workshop_id: None,
                    l4d2center_name: Some(entry.name.clone()),
                    title: entry.name.clone(),
                    preview_url: None,
                    download_link: Some(entry.download_link.clone()),
                    size_bytes: Some(entry.size),
                    proposed_by,
                    proposed_at: Utc::now(),
                }
            }
            ResolvedInstallTarget::Url { .. } => {
                return Err((
                    Status::BadRequest,
                    "Only workshop and L4D2Center suggestions are supported".to_string(),
                ));
            }
        };

        let store = self.suggestions_store();
        if store
            .find_duplicate(suggestion.workshop_id, suggestion.l4d2center_name.as_deref())
            .map_err(|e| (Status::InternalServerError, e))?
            .is_some()
        {
            return Err((
                Status::Conflict,
                "This map is already suggested".to_string(),
            ));
        }

        let maps = self
            .load_installed_maps()
            .map_err(|e| (Status::InternalServerError, e))?;
        match check_installed_conflict(&suggestion, &maps) {
            ConflictCheck::SameSource { installed } => {
                return Err((
                    Status::Conflict,
                    format!(
                        "Map already installed as '{}' (#{})",
                        installed.name, installed.id
                    ),
                ));
            }
            ConflictCheck::OtherSource { .. } | ConflictCheck::None => {}
        }

        let created = store
            .create(suggestion)
            .map_err(|e| (Status::InternalServerError, e))?;
        let conflict = conflict_to_view(&check_installed_conflict(&created, &maps));
        Ok(MapSuggestionView {
            suggestion: created,
            conflict,
        })
    }

    pub async fn deny_map_suggestion(&self, id: u64) -> Result<(), (Status, String)> {
        let removed = self
            .suggestions_store()
            .delete(id)
            .map_err(|e| (Status::InternalServerError, e))?;
        if !removed {
            return Err((Status::NotFound, format!("Suggestion #{id} not found")));
        }
        Ok(())
    }

    pub async fn accept_map_suggestion(
        &self,
        id: u64,
        api_key: Option<&str>,
    ) -> Result<AcceptMapSuggestionResponse, (Status, String)> {
        if self.daemon_url.trim().is_empty() {
            return Err((
                Status::BadGateway,
                "Daemon URL not configured".to_string(),
            ));
        }

        let store = self.suggestions_store();
        let suggestion = store
            .get(id)
            .map_err(|e| (Status::InternalServerError, e))?
            .ok_or_else(|| (Status::NotFound, format!("Suggestion #{id} not found")))?;

        let maps = self
            .load_installed_maps()
            .map_err(|e| (Status::InternalServerError, e))?;
        if let ConflictCheck::SameSource { installed } =
            check_installed_conflict(&suggestion, &maps)
        {
            return Err((
                Status::Conflict,
                format!(
                    "Map already installed as '{}' (#{})",
                    installed.name, installed.id
                ),
            ));
        }

        let map_id = match suggestion.source_kind {
            MapSuggestionSourceKind::Workshop => {
                let workshop_id = suggestion.workshop_id.ok_or_else(|| {
                    (
                        Status::InternalServerError,
                        "Suggestion missing workshop_id".to_string(),
                    )
                })?;
                proxy_daemon_install_map(
                    &self.install_http_client,
                    &self.daemon_url,
                    self.sync_api_key.as_deref(),
                    &ResolvedInstallTarget::Workshop { workshop_id },
                    Some(suggestion.title.clone()),
                )
                .await
                .map_err(|e| (Status::BadGateway, e))?
            }
            MapSuggestionSourceKind::L4d2Center => {
                let name = suggestion.l4d2center_name.as_deref().ok_or_else(|| {
                    (
                        Status::InternalServerError,
                        "Suggestion missing l4d2center_name".to_string(),
                    )
                })?;
                proxy_daemon_l4d2center_install(
                    &self.install_http_client,
                    &self.daemon_url,
                    self.sync_api_key.as_deref(),
                    name,
                )
                .await
                .map_err(|e| (Status::BadGateway, e))?
            }
        };

        let _ = store.delete(id);
        if let Err(error) = self.refresh_registry_from_daemon(api_key).await {
            eprintln!(
                "Registry refresh after accepting map suggestion failed (non-fatal): {error}"
            );
        }

        Ok(AcceptMapSuggestionResponse { map_id })
    }
}

#[get("/maps/suggestions")]
pub async fn list_map_suggestions(
    bridge: &State<MapsBridgeState>,
) -> Result<Json<Vec<MapSuggestionView>>, (Status, Json<AdminInstallErrorResponse>)> {
    bridge
        .list_map_suggestions()
        .await
        .map(Json)
        .map_err(|e| suggestion_error(Status::InternalServerError, e))
}

#[post("/maps/suggestions", data = "<body>")]
pub async fn create_map_suggestion(
    user: AuthUser,
    bridge: &State<MapsBridgeState>,
    config_handle: &State<ConfigHandle>,
    body: Json<CreateMapSuggestionRequest>,
) -> Result<Json<MapSuggestionView>, (Status, Json<AdminInstallErrorResponse>)> {
    let api_key = steam_api_key(config_handle);
    bridge
        .create_map_suggestion(
            &body.mode,
            &body.input,
            user.0.to_string(),
            api_key.as_deref(),
        )
        .await
        .map(Json)
        .map_err(|(status, error)| suggestion_error(status, error))
}

#[post("/maps/suggestions/<id>/deny")]
pub async fn deny_map_suggestion(
    _admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    id: u64,
) -> Result<Status, (Status, Json<AdminInstallErrorResponse>)> {
    bridge
        .deny_map_suggestion(id)
        .await
        .map(|_| Status::Ok)
        .map_err(|(status, error)| suggestion_error(status, error))
}

#[post("/maps/suggestions/<id>/accept")]
pub async fn accept_map_suggestion(
    _admin: AdminUser,
    bridge: &State<MapsBridgeState>,
    config_handle: &State<ConfigHandle>,
    id: u64,
) -> Result<Json<AcceptMapSuggestionResponse>, (Status, Json<AdminInstallErrorResponse>)> {
    let api_key = steam_api_key(config_handle);
    bridge
        .accept_map_suggestion(id, api_key.as_deref())
        .await
        .map(Json)
        .map_err(|(status, error)| suggestion_error(status, error))
}

#[options("/maps/suggestions")]
pub fn options_map_suggestions() -> Status {
    Status::Ok
}

#[options("/maps/suggestions/<id>/deny")]
pub fn options_deny_map_suggestion(id: u64) -> Status {
    let _ = id;
    Status::Ok
}

#[options("/maps/suggestions/<id>/accept")]
pub fn options_accept_map_suggestion(id: u64) -> Status {
    let _ = id;
    Status::Ok
}
