// SPDX-License-Identifier: GPL-3.0-only

use std::path::{Path, PathBuf};

use chrono::{DateTime, Utc};
use rocket::serde::{Deserialize, Serialize};

use super::models::DaemonMapEntry;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
pub struct RegistryFile {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub last_synced_at: Option<DateTime<Utc>>,
    pub maps: Vec<DaemonMapEntry>,
}

impl Default for RegistryFile {
    fn default() -> Self {
        Self {
            last_synced_at: None,
            maps: Vec::new(),
        }
    }
}

pub fn load_registry(path: &Path) -> Result<RegistryFile, String> {
    if !path.exists() {
        return Ok(RegistryFile::default());
    }

    let content = std::fs::read_to_string(path)
        .map_err(|e| format!("Failed to read registry {}: {}", path.display(), e))?;

    rocket::serde::json::from_str(&content)
        .map_err(|e| format!("Failed to parse registry {}: {}", path.display(), e))
}

pub fn save_registry(path: &Path, maps: Vec<DaemonMapEntry>) -> Result<(), String> {
    let file = RegistryFile {
        last_synced_at: Some(Utc::now()),
        maps,
    };
    save_registry_file(path, &file)
}

pub fn save_registry_file(path: &Path, file: &RegistryFile) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .map_err(|e| format!("Failed to create registry directory: {}", e))?;
    }

    let json = rocket::serde::json::to_pretty_string(file)
        .map_err(|e| format!("Failed to serialize registry: {}", e))?;

    let temp_path = path.with_extension("tmp");
    std::fs::write(&temp_path, json)
        .map_err(|e| format!("Failed to write temp registry: {}", e))?;
    std::fs::rename(&temp_path, path)
        .map_err(|e| format!("Failed to rename registry file: {}", e))?;

    Ok(())
}

pub fn resolve_data_path(base: &Path, configured: &str, default_name: &str) -> PathBuf {
    if configured.trim().is_empty() {
        base.join(default_name)
    } else {
        let path = PathBuf::from(configured);
        if path.is_absolute() {
            path
        } else {
            base.join(path)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps_bridge::models::{DaemonMapEntry, DaemonSourceKind};

    fn sample_entry() -> DaemonMapEntry {
        DaemonMapEntry {
            id: 1,
            name: "Test Map".to_string(),
            source_url: "https://example.com/map.zip".to_string(),
            source_kind: DaemonSourceKind::Other,
            workshop_id: None,
            installed_path: "test.vpk".to_string(),
            installed_at: Utc::now(),
            workshop_updated_at: None,
            version: None,
            checksum: None,
            checksum_kind: None,
        }
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("maps_registry.json");

        save_registry(&path, vec![sample_entry()]).expect("save");
        let loaded = load_registry(&path).expect("load");
        assert_eq!(loaded.maps.len(), 1);
        assert_eq!(loaded.maps[0].name, "Test Map");
        assert!(loaded.last_synced_at.is_some());
    }

    #[test]
    fn load_missing_returns_empty() {
        let dir = tempfile::tempdir().expect("tempdir");
        let path = dir.path().join("missing.json");
        let loaded = load_registry(&path).expect("load");
        assert!(loaded.maps.is_empty());
    }
}
