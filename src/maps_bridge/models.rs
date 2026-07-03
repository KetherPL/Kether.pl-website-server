// SPDX-License-Identifier: GPL-3.0-only

use chrono::{DateTime, Utc};
use rocket::serde::{Deserialize, Serialize};

/// Mirrors KetherServerDaemon `SourceKind`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(crate = "rocket::serde", rename_all = "lowercase")]
pub enum DaemonSourceKind {
    Workshop,
    SirPlease,
    L4d2Center,
    Other,
}

/// Mirrors KetherServerDaemon `MapEntry` (1:1 registry payload).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct DaemonMapEntry {
    pub id: u64,
    pub name: String,
    pub source_url: String,
    pub source_kind: DaemonSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workshop_id: Option<u64>,
    pub installed_path: String,
    pub installed_at: DateTime<Utc>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workshop_updated_at: Option<DateTime<Utc>>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub checksum_kind: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct SyncRequest {
    pub maps: Vec<DaemonMapEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct MapUpdate {
    pub action: String,
    pub map_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map_entry: Option<DaemonMapEntry>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct UpdatesResponse {
    pub updates: Vec<MapUpdate>,
}

/// Website-facing map list entry.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct WebsiteMapEntry {
    pub mapName: String,
    pub source: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub downloadUrl: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub previewUrl: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub enum MapsDataSource {
    #[serde(rename = "registry")]
    Registry,
    #[serde(rename = "registry_stale")]
    RegistryStale,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct MapsListResponse {
    pub maps: Vec<WebsiteMapEntry>,
    pub stale: bool,
    pub source: MapsDataSource,
}

/// Daemon `GET /api/maps` envelope.
#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct DaemonApiResponse {
    pub success: bool,
    #[serde(default)]
    pub data: Option<Vec<DaemonMapEntry>>,
    #[serde(default)]
    pub error: Option<String>,
}

pub fn daemon_source_to_website(kind: DaemonSourceKind) -> &'static str {
    match kind {
        DaemonSourceKind::Workshop => "Workshop",
        DaemonSourceKind::SirPlease => "SirPlease",
        DaemonSourceKind::L4d2Center => "L4D2Center",
        DaemonSourceKind::Other => "Other",
    }
}

pub fn workshop_download_url(workshop_id: u64) -> String {
    format!(
        "https://steamcommunity.com/sharedfiles/filedetails/?id={}",
        workshop_id
    )
}
