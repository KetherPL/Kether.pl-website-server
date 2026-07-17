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
    pub id: u64,
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

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminMapDetailResponse {
    pub id: u64,
    pub name: String,
    pub installed_path: String,
    pub source_kind: DaemonSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub workshop_id: Option<u64>,
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

impl From<DaemonMapEntry> for AdminMapDetailResponse {
    fn from(entry: DaemonMapEntry) -> Self {
        Self {
            id: entry.id,
            name: entry.name,
            installed_path: entry.installed_path,
            source_kind: entry.source_kind,
            workshop_id: entry.workshop_id,
            installed_at: entry.installed_at,
            workshop_updated_at: entry.workshop_updated_at,
            version: entry.version,
            checksum: entry.checksum,
            checksum_kind: entry.checksum_kind,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(crate = "rocket::serde", rename_all = "snake_case")]
pub enum UpdateStatus {
    Updated,
    UpToDate,
    Unsupported,
    Failed,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminUpdateCheckResponse {
    pub status: UpdateStatus,
    pub message: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub map: Option<AdminMapDetailResponse>,
}

/// Compact pending/in-progress update entry from the daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminMapUpdateItem {
    pub name: String,
    pub map_id: u64,
    pub source_kind: DaemonSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub phase: Option<String>,
    #[serde(default)]
    pub bytes_downloaded: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bytes_total: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub percent: Option<u8>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminMapUpdatesStatus {
    pub available: Vec<AdminMapUpdateItem>,
    pub in_progress: Vec<AdminMapUpdateItem>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminApplyUpdatesRequest {
    #[serde(default)]
    pub map_id: Option<u64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct AdminApplyUpdatesResponse {
    pub results: Vec<AdminUpdateCheckResponse>,
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
