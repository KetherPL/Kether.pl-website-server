// SPDX-License-Identifier: GPL-3.0-only

use super::models::{
    daemon_source_to_website, workshop_download_url, DaemonMapEntry, WebsiteMapEntry,
};

pub fn daemon_entry_to_website(entry: &DaemonMapEntry) -> WebsiteMapEntry {
    let source = daemon_source_to_website(entry.source_kind).to_string();
    let download_url = download_url_for_entry(entry);

    WebsiteMapEntry {
        mapName: entry.name.clone(),
        source,
        downloadUrl: download_url,
        previewUrl: None,
    }
}

fn download_url_for_entry(entry: &DaemonMapEntry) -> Option<String> {
    if !entry.source_url.trim().is_empty() {
        return Some(entry.source_url.clone());
    }

    entry.workshop_id.map(workshop_download_url)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::maps_bridge::models::DaemonSourceKind;

    #[test]
    fn maps_workshop_id_to_download_url_when_source_url_empty() {
        let entry = DaemonMapEntry {
            id: 1,
            name: "Test".to_string(),
            source_url: String::new(),
            source_kind: DaemonSourceKind::Workshop,
            workshop_id: Some(381419931),
            installed_path: "test.vpk".to_string(),
            installed_at: chrono::Utc::now(),
            workshop_updated_at: None,
            version: None,
            checksum: None,
            checksum_kind: None,
        };

        let mapped = daemon_entry_to_website(&entry);
        assert_eq!(
            mapped.downloadUrl.as_deref(),
            Some("https://steamcommunity.com/sharedfiles/filedetails/?id=381419931")
        );
        assert!(mapped.previewUrl.is_none());
    }
}
