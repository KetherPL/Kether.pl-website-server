// SPDX-License-Identifier: GPL-3.0-only

use super::models::{
    daemon_source_to_website, DaemonMapEntry, DaemonSourceKind, MapSuggestion,
    MapSuggestionConflict, MapSuggestionSourceKind,
};

/// Normalize a map title/path stem the way the daemon does for filenames.
pub fn normalize_map_stem(raw: &str) -> String {
    let sanitized: String = raw
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == '-' || *c == '_' || *c == ' ' || *c == '.')
        .collect();
    sanitized
        .to_lowercase()
        .trim()
        .replace(' ', "_")
        .trim_end_matches(".vpk")
        .to_string()
}

fn path_stem(path: &str) -> String {
    let name = path.rsplit('/').next().unwrap_or(path);
    normalize_map_stem(name)
}

#[derive(Debug, Clone)]
pub enum ConflictCheck {
    SameSource {
        installed: DaemonMapEntry,
    },
    OtherSource {
        installed: DaemonMapEntry,
    },
    None,
}

pub fn check_installed_conflict(
    suggestion: &MapSuggestion,
    maps: &[DaemonMapEntry],
) -> ConflictCheck {
    match suggestion.source_kind {
        MapSuggestionSourceKind::Workshop => {
            let Some(workshop_id) = suggestion.workshop_id else {
                return ConflictCheck::None;
            };
            if let Some(existing) = maps.iter().find(|m| m.workshop_id == Some(workshop_id)) {
                return ConflictCheck::SameSource {
                    installed: existing.clone(),
                };
            }
            let title_stem = normalize_map_stem(&suggestion.title);
            if title_stem.is_empty() {
                return ConflictCheck::None;
            }
            if let Some(existing) = maps.iter().find(|m| {
                m.source_kind != DaemonSourceKind::Workshop
                    && (path_stem(&m.installed_path) == title_stem
                        || normalize_map_stem(&m.name) == title_stem)
            }) {
                return ConflictCheck::OtherSource {
                    installed: existing.clone(),
                };
            }
            ConflictCheck::None
        }
        MapSuggestionSourceKind::L4d2Center => {
            let Some(catalog_name) = suggestion.l4d2center_name.as_deref() else {
                return ConflictCheck::None;
            };
            let catalog_stem = normalize_map_stem(catalog_name);
            if let Some(existing) = maps.iter().find(|m| {
                m.source_kind == DaemonSourceKind::L4d2Center
                    && (m.installed_path.eq_ignore_ascii_case(catalog_name)
                        || path_stem(&m.installed_path) == catalog_stem
                        || suggestion
                            .download_link
                            .as_deref()
                            .is_some_and(|link| m.source_url == *link))
            }) {
                return ConflictCheck::SameSource {
                    installed: existing.clone(),
                };
            }
            if let Some(existing) = maps.iter().find(|m| {
                m.source_kind != DaemonSourceKind::L4d2Center
                    && (m.installed_path.eq_ignore_ascii_case(catalog_name)
                        || path_stem(&m.installed_path) == catalog_stem)
            }) {
                return ConflictCheck::OtherSource {
                    installed: existing.clone(),
                };
            }
            ConflictCheck::None
        }
    }
}

pub fn conflict_to_view(check: &ConflictCheck) -> Option<MapSuggestionConflict> {
    match check {
        ConflictCheck::OtherSource { installed } => Some(MapSuggestionConflict {
            kind: "other_source".to_string(),
            installed_map_id: installed.id,
            installed_name: installed.name.clone(),
            installed_source: daemon_source_to_website(installed.source_kind).to_string(),
        }),
        ConflictCheck::SameSource { .. } | ConflictCheck::None => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Utc;

    fn entry(
        id: u64,
        name: &str,
        kind: DaemonSourceKind,
        path: &str,
        workshop_id: Option<u64>,
    ) -> DaemonMapEntry {
        DaemonMapEntry {
            id,
            name: name.to_string(),
            source_url: String::new(),
            source_kind: kind,
            workshop_id,
            installed_path: path.to_string(),
            installed_at: Utc::now(),
            workshop_updated_at: None,
            version: None,
            checksum: None,
            checksum_kind: None,
        }
    }

    #[test]
    fn workshop_same_source_conflict() {
        let maps = vec![entry(
            1,
            "foo",
            DaemonSourceKind::Workshop,
            "foo.vpk",
            Some(99),
        )];
        let suggestion = MapSuggestion {
            id: 0,
            source_kind: MapSuggestionSourceKind::Workshop,
            workshop_id: Some(99),
            l4d2center_name: None,
            title: "Foo".to_string(),
            preview_url: None,
            download_link: None,
            size_bytes: None,
            proposed_by: "1".to_string(),
            proposed_at: Utc::now(),
        };
        assert!(matches!(
            check_installed_conflict(&suggestion, &maps),
            ConflictCheck::SameSource { .. }
        ));
    }

    #[test]
    fn workshop_other_source_by_title() {
        let maps = vec![entry(
            2,
            "dark carnival",
            DaemonSourceKind::L4d2Center,
            "dark_carnival.vpk",
            None,
        )];
        let suggestion = MapSuggestion {
            id: 0,
            source_kind: MapSuggestionSourceKind::Workshop,
            workshop_id: Some(1),
            l4d2center_name: None,
            title: "Dark Carnival".to_string(),
            preview_url: None,
            download_link: None,
            size_bytes: None,
            proposed_by: "1".to_string(),
            proposed_at: Utc::now(),
        };
        assert!(matches!(
            check_installed_conflict(&suggestion, &maps),
            ConflictCheck::OtherSource { .. }
        ));
    }

    #[test]
    fn l4d2center_other_source_by_path() {
        let maps = vec![entry(
            3,
            "widebox",
            DaemonSourceKind::Workshop,
            "widebox1.vpk",
            Some(5),
        )];
        let suggestion = MapSuggestion {
            id: 0,
            source_kind: MapSuggestionSourceKind::L4d2Center,
            workshop_id: None,
            l4d2center_name: Some("widebox1.vpk".to_string()),
            title: "widebox1.vpk".to_string(),
            preview_url: None,
            download_link: None,
            size_bytes: None,
            proposed_by: "1".to_string(),
            proposed_at: Utc::now(),
        };
        assert!(matches!(
            check_installed_conflict(&suggestion, &maps),
            ConflictCheck::OtherSource { .. }
        ));
    }
}
