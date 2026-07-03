// SPDX-License-Identifier: GPL-3.0-only

use super::models::{
    daemon_source_to_website, workshop_download_url, DaemonMapEntry, DaemonSourceKind,
    WebsiteMapEntry,
};
use super::workshop_previews::WorkshopPreviewCache;

pub fn daemon_entry_to_website(
    entry: &DaemonMapEntry,
    previews: &WorkshopPreviewCache,
) -> WebsiteMapEntry {
    let source = daemon_source_to_website(entry.source_kind).to_string();
    let download_url = download_url_for_entry(entry);
    let preview_url = preview_url_for_entry(entry, previews);

    WebsiteMapEntry {
        mapName: entry.name.clone(),
        source,
        downloadUrl: download_url,
        previewUrl: preview_url,
    }
}

fn download_url_for_entry(entry: &DaemonMapEntry) -> Option<String> {
    if !entry.source_url.trim().is_empty() {
        return Some(entry.source_url.clone());
    }

    entry.workshop_id.map(workshop_download_url)
}

fn preview_url_for_entry(entry: &DaemonMapEntry, previews: &WorkshopPreviewCache) -> Option<String> {
    if entry.source_kind != DaemonSourceKind::Workshop {
        return None;
    }

    entry
        .workshop_id
        .and_then(|id| previews.preview_url(id).map(str::to_string))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps_bridge::workshop_previews::{PreviewCacheEntry, WorkshopPreviewCache};
    use chrono::Utc;

    fn workshop_entry(workshop_id: u64) -> DaemonMapEntry {
        DaemonMapEntry {
            id: 1,
            name: "Test".to_string(),
            source_url: String::new(),
            source_kind: DaemonSourceKind::Workshop,
            workshop_id: Some(workshop_id),
            installed_path: "test.vpk".to_string(),
            installed_at: Utc::now(),
            workshop_updated_at: None,
            version: None,
            checksum: None,
            checksum_kind: None,
        }
    }

    #[test]
    fn maps_workshop_id_to_download_url_when_source_url_empty() {
        let entry = workshop_entry(381419931);
        let cache = WorkshopPreviewCache::default();

        let mapped = daemon_entry_to_website(&entry, &cache);
        assert_eq!(
            mapped.downloadUrl.as_deref(),
            Some("https://steamcommunity.com/sharedfiles/filedetails/?id=381419931")
        );
        assert!(mapped.previewUrl.is_none());
    }

    #[test]
    fn workshop_entry_uses_cached_preview_url() {
        let entry = workshop_entry(381419931);
        let mut cache = WorkshopPreviewCache::default();
        cache.insert(
            381419931,
            PreviewCacheEntry {
                preview_url: "https://images.steamusercontent.com/ugc/test/".to_string(),
                workshop_updated_at: None,
                fetched_at: Utc::now(),
            },
        );

        let mapped = daemon_entry_to_website(&entry, &cache);
        assert_eq!(
            mapped.previewUrl.as_deref(),
            Some("https://images.steamusercontent.com/ugc/test/")
        );
    }

    #[test]
    fn non_workshop_entry_has_no_preview_url() {
        let entry = DaemonMapEntry {
            id: 1,
            name: "SirPlease".to_string(),
            source_url: "https://example.com/map.zip".to_string(),
            source_kind: DaemonSourceKind::SirPlease,
            workshop_id: None,
            installed_path: "map.vpk".to_string(),
            installed_at: Utc::now(),
            workshop_updated_at: None,
            version: None,
            checksum: None,
            checksum_kind: None,
        };

        let mut cache = WorkshopPreviewCache::default();
        cache.insert(
            123,
            PreviewCacheEntry {
                preview_url: "https://images.steamusercontent.com/ugc/test/".to_string(),
                workshop_updated_at: None,
                fetched_at: Utc::now(),
            },
        );

        let mapped = daemon_entry_to_website(&entry, &cache);
        assert!(mapped.previewUrl.is_none());
    }
}
