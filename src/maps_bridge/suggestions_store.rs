// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

use chrono::{DateTime, Utc};
use rocket::serde::{Deserialize, Serialize};

use super::models::{MapSuggestion, MapSuggestionSourceKind};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(crate = "rocket::serde")]
struct SuggestionsFile {
    #[serde(default)]
    next_id: u64,
    #[serde(default)]
    suggestions: HashMap<String, StoredSuggestion>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct StoredSuggestion {
    source_kind: MapSuggestionSourceKind,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    workshop_id: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    l4d2center_name: Option<String>,
    title: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    preview_url: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    download_link: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    size_bytes: Option<u64>,
    proposed_by: String,
    proposed_at: DateTime<Utc>,
}

pub struct SuggestionsStore {
    path: PathBuf,
    lock: Mutex<()>,
}

impl SuggestionsStore {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            lock: Mutex::new(()),
        }
    }

    pub fn path_from_registry(registry_path: &Path) -> PathBuf {
        registry_path
            .parent()
            .map(|parent| parent.join("map_suggestions.json"))
            .unwrap_or_else(|| PathBuf::from("map_suggestions.json"))
    }

    fn load(&self) -> Result<SuggestionsFile, String> {
        if !self.path.exists() {
            return Ok(SuggestionsFile::default());
        }
        let content = std::fs::read_to_string(&self.path).map_err(|e| {
            format!(
                "Failed to read suggestions {}: {}",
                self.path.display(),
                e
            )
        })?;
        rocket::serde::json::from_str(&content).map_err(|e| {
            format!(
                "Failed to parse suggestions {}: {}",
                self.path.display(),
                e
            )
        })
    }

    fn save(&self, file: &SuggestionsFile) -> Result<(), String> {
        if let Some(parent) = self.path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| format!("Failed to create suggestions directory: {e}"))?;
        }
        let json = rocket::serde::json::to_pretty_string(file)
            .map_err(|e| format!("Failed to serialize suggestions: {e}"))?;
        let temp_path = self.path.with_extension("tmp");
        std::fs::write(&temp_path, json)
            .map_err(|e| format!("Failed to write temp suggestions: {e}"))?;
        std::fs::rename(&temp_path, &self.path)
            .map_err(|e| format!("Failed to rename suggestions file: {e}"))?;
        Ok(())
    }

    fn to_public(id: u64, data: &StoredSuggestion) -> MapSuggestion {
        MapSuggestion {
            id,
            source_kind: data.source_kind,
            workshop_id: data.workshop_id,
            l4d2center_name: data.l4d2center_name.clone(),
            title: data.title.clone(),
            preview_url: data.preview_url.clone(),
            download_link: data.download_link.clone(),
            size_bytes: data.size_bytes,
            proposed_by: data.proposed_by.clone(),
            proposed_at: data.proposed_at,
        }
    }

    pub fn list(&self) -> Result<Vec<MapSuggestion>, String> {
        let _guard = self.lock.lock().map_err(|_| "suggestions lock poisoned")?;
        let file = self.load()?;
        let mut items: Vec<MapSuggestion> = file
            .suggestions
            .iter()
            .filter_map(|(id_str, data)| {
                let id = id_str.parse::<u64>().ok()?;
                Some(Self::to_public(id, data))
            })
            .collect();
        items.sort_by(|a, b| b.proposed_at.cmp(&a.proposed_at));
        Ok(items)
    }

    pub fn get(&self, id: u64) -> Result<Option<MapSuggestion>, String> {
        let _guard = self.lock.lock().map_err(|_| "suggestions lock poisoned")?;
        let file = self.load()?;
        Ok(file
            .suggestions
            .get(&id.to_string())
            .map(|data| Self::to_public(id, data)))
    }

    pub fn find_duplicate(
        &self,
        workshop_id: Option<u64>,
        l4d2center_name: Option<&str>,
    ) -> Result<Option<MapSuggestion>, String> {
        let items = self.list()?;
        Ok(items.into_iter().find(|item| {
            if let (Some(want), Some(have)) = (workshop_id, item.workshop_id) {
                return want == have;
            }
            if let (Some(want), Some(have)) = (l4d2center_name, item.l4d2center_name.as_deref()) {
                return want.eq_ignore_ascii_case(have);
            }
            false
        }))
    }

    pub fn create(&self, mut suggestion: MapSuggestion) -> Result<MapSuggestion, String> {
        let _guard = self.lock.lock().map_err(|_| "suggestions lock poisoned")?;
        let mut file = self.load()?;
        let id = file.next_id.max(1);
        file.next_id = id + 1;
        suggestion.id = id;
        file.suggestions.insert(
            id.to_string(),
            StoredSuggestion {
                source_kind: suggestion.source_kind,
                workshop_id: suggestion.workshop_id,
                l4d2center_name: suggestion.l4d2center_name.clone(),
                title: suggestion.title.clone(),
                preview_url: suggestion.preview_url.clone(),
                download_link: suggestion.download_link.clone(),
                size_bytes: suggestion.size_bytes,
                proposed_by: suggestion.proposed_by.clone(),
                proposed_at: suggestion.proposed_at,
            },
        );
        self.save(&file)?;
        Ok(suggestion)
    }

    pub fn delete(&self, id: u64) -> Result<bool, String> {
        let _guard = self.lock.lock().map_err(|_| "suggestions lock poisoned")?;
        let mut file = self.load()?;
        let removed = file.suggestions.remove(&id.to_string()).is_some();
        if removed {
            self.save(&file)?;
        }
        Ok(removed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::tempdir;

    #[test]
    fn create_list_delete_roundtrip() {
        let dir = tempdir().unwrap();
        let store = SuggestionsStore::new(dir.path().join("map_suggestions.json"));
        let created = store
            .create(MapSuggestion {
                id: 0,
                source_kind: MapSuggestionSourceKind::Workshop,
                workshop_id: Some(123),
                l4d2center_name: None,
                title: "Test".to_string(),
                preview_url: None,
                download_link: None,
                size_bytes: None,
                proposed_by: "1".to_string(),
                proposed_at: Utc::now(),
            })
            .unwrap();
        assert_eq!(created.id, 1);
        assert_eq!(store.list().unwrap().len(), 1);
        assert!(store.delete(1).unwrap());
        assert!(store.list().unwrap().is_empty());
    }
}
