// SPDX-License-Identifier: GPL-3.0-only

use reqwest::Client;
use rocket::serde::Deserialize;

pub const L4D2CENTER_INDEX_URL: &str = "https://l4d2center.com/maps/servers/index.json";

#[derive(Debug, Clone, Deserialize, PartialEq, Eq)]
#[serde(crate = "rocket::serde")]
pub struct L4d2CenterIndexEntry {
    pub name: String,
    pub size: u64,
    #[serde(default)]
    pub md5: String,
    pub download_link: String,
}

fn normalize_catalog_key(name: &str) -> String {
    name.trim().to_lowercase().trim_end_matches(".vpk").to_string()
}

pub fn find_l4d2center_entry<'a>(
    entries: &'a [L4d2CenterIndexEntry],
    input: &str,
) -> Option<&'a L4d2CenterIndexEntry> {
    let trimmed = input.trim();
    if trimmed.is_empty() {
        return None;
    }
    if let Some(exact) = entries
        .iter()
        .find(|e| e.name.eq_ignore_ascii_case(trimmed))
    {
        return Some(exact);
    }
    let key = normalize_catalog_key(trimmed);
    entries
        .iter()
        .find(|e| normalize_catalog_key(&e.name) == key)
}

pub async fn fetch_l4d2center_index(client: &Client) -> Result<Vec<L4d2CenterIndexEntry>, String> {
    let response = client
        .get(L4D2CENTER_INDEX_URL)
        .send()
        .await
        .map_err(|e| format!("L4D2Center index request failed: {e}"))?;
    if !response.status().is_success() {
        return Err(format!(
            "L4D2Center index returned HTTP {}",
            response.status()
        ));
    }
    let text = response
        .text()
        .await
        .map_err(|e| format!("Failed to read L4D2Center index: {e}"))?;
    rocket::serde::json::from_str(&text)
        .map_err(|e| format!("Failed to parse L4D2Center index: {e}"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn find_by_stem() {
        let entries = vec![L4d2CenterIndexEntry {
            name: "widebox1.vpk".to_string(),
            size: 10,
            md5: "abc".to_string(),
            download_link: "https://example.com/widebox1.7z".to_string(),
        }];
        assert_eq!(
            find_l4d2center_entry(&entries, "WideBox1")
                .unwrap()
                .name,
            "widebox1.vpk"
        );
    }
}
