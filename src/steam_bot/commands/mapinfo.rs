// SPDX-License-Identifier: GPL-3.0-only

use super::{CommandContext, CommandError, CommandHandler, CommandInfo, CommandMetadata};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use chrono_tz::Europe::Warsaw;

const MAPINFO_USAGE: &str = "!mapinfo <map name> | !mi <map name>";
const MAPINFO_DESCRIPTION: &str =
    "Shows an installed map's checksum, last update (or install) time and workshop ID / URL.";

/// Map details needed to build the `!mapinfo` reply (decoupled from the maps bridge types).
struct MapInfo {
    map_name: String,
    source: String,
    checksum: Option<String>,
    checksum_kind: Option<String>,
    workshop_updated_at: Option<DateTime<Utc>>,
    installed_at: DateTime<Utc>,
    workshop_id: Option<u64>,
    url: Option<String>,
}

#[cfg(feature = "maps_bridge")]
impl From<crate::maps_bridge::InstalledMapInfo> for MapInfo {
    fn from(info: crate::maps_bridge::InstalledMapInfo) -> Self {
        Self {
            map_name: info.map_name,
            source: info.source,
            checksum: info.checksum,
            checksum_kind: info.checksum_kind,
            workshop_updated_at: info.workshop_updated_at,
            installed_at: info.installed_at,
            workshop_id: info.workshop_id,
            url: info.url,
        }
    }
}

/// Shows details of a single installed map.
struct MapInfoCommand;

impl MapInfoCommand {
    fn parse_map_name(args: &str) -> Result<&str, CommandError> {
        let trimmed = args.trim();
        if trimmed.is_empty() {
            return Err(CommandError::InvalidArguments(format!(
                "Usage: {}",
                MAPINFO_USAGE
            )));
        }
        Ok(trimmed)
    }

    /// Formats a timestamp in CET/CEST, e.g. `2026-10-19 21:03 CEST`.
    fn format_time(time: DateTime<Utc>) -> String {
        time.with_timezone(&Warsaw).format("%Y-%m-%d %H:%M %Z").to_string()
    }

    fn format_info(info: &MapInfo) -> String {
        let non_empty = |value: &Option<String>| {
            value
                .as_deref()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_string)
        };

        let checksum_line = match (non_empty(&info.checksum), non_empty(&info.checksum_kind)) {
            (Some(checksum), Some(kind)) => format!("Checksum ({}): {}", kind, checksum),
            (Some(checksum), None) => format!("Checksum (unknown type): {}", checksum),
            (None, _) => "Checksum: n/a".to_string(),
        };

        let time_line = match info.workshop_updated_at {
            Some(updated) => format!("Last updated: {}", Self::format_time(updated)),
            None => format!("Installed: {}", Self::format_time(info.installed_at)),
        };

        let id_line = match (info.workshop_id, non_empty(&info.url)) {
            (Some(id), _) => format!("Workshop ID: {}", id),
            (None, Some(url)) => format!("URL: {}", url),
            (None, None) => "URL: n/a".to_string(),
        };

        format!(
            "{} ({})\n{}\n{}\n{}",
            info.map_name, info.source, checksum_line, time_line, id_line
        )
    }
}

#[async_trait]
impl CommandHandler for MapInfoCommand {
    #[cfg(feature = "maps_bridge")]
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        let map_name = Self::parse_map_name(ctx.args)?;

        let base_path = crate::config::exe_dir().map_err(|e| {
            eprintln!("!mapinfo: failed to resolve exe dir: {}", e);
            CommandError::ServerError("Failed to load map info.".to_string())
        })?;
        let config = crate::steam_bot::registry::config();

        Ok(
            match crate::maps_bridge::find_installed_map_info(&base_path, &config, map_name) {
                Some(info) => Self::format_info(&MapInfo::from(info)),
                None => format!("No installed map found matching \"{}\".", map_name),
            },
        )
    }

    #[cfg(not(feature = "maps_bridge"))]
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        Self::parse_map_name(ctx.args)?;
        Ok("Map info is not available on this server.".to_string())
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "mapinfo",
            aliases: &["mi"],
            description: MAPINFO_DESCRIPTION,
            usage: Some(MAPINFO_USAGE),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "mapinfo",
        &["mi"],
        MAPINFO_DESCRIPTION,
        Some(MAPINFO_USAGE),
        || Box::new(MapInfoCommand)
    )
}

#[cfg(test)]
mod tests {
    use super::{MapInfo, MapInfoCommand};
    use crate::steam_bot::commands::CommandError;
    use chrono::{TimeZone, Utc};

    fn workshop_info() -> MapInfo {
        MapInfo {
            map_name: "Some Map".to_string(),
            source: "Workshop".to_string(),
            checksum: Some("abc123".to_string()),
            checksum_kind: Some("md5".to_string()),
            // 19:03 UTC in summer -> 21:03 CEST
            workshop_updated_at: Some(Utc.with_ymd_and_hms(2026, 7, 1, 19, 3, 0).unwrap()),
            installed_at: Utc.with_ymd_and_hms(2026, 1, 1, 12, 0, 0).unwrap(),
            workshop_id: Some(381419931),
            url: Some("https://steamcommunity.com/sharedfiles/filedetails/?id=381419931".to_string()),
        }
    }

    #[test]
    fn parse_map_name_requires_argument() {
        match MapInfoCommand::parse_map_name("  ") {
            Err(CommandError::InvalidArguments(msg)) => assert!(msg.contains("!mapinfo")),
            other => panic!("expected InvalidArguments, got {:?}", other),
        }
        assert_eq!(MapInfoCommand::parse_map_name(" No Mercy ").unwrap(), "No Mercy");
    }

    #[test]
    fn workshop_map_shows_update_time_and_workshop_id() {
        assert_eq!(
            MapInfoCommand::format_info(&workshop_info()),
            "Some Map (Workshop)\n\
             Checksum (md5): abc123\n\
             Last updated: 2026-07-01 21:03 CEST\n\
             Workshop ID: 381419931"
        );
    }

    #[test]
    fn non_workshop_map_falls_back_to_install_time_and_url() {
        let info = MapInfo {
            source: "L4D2Center".to_string(),
            checksum_kind: None,
            workshop_updated_at: None,
            workshop_id: None,
            url: Some("https://example.com/map.zip".to_string()),
            ..workshop_info()
        };
        assert_eq!(
            MapInfoCommand::format_info(&info),
            "Some Map (L4D2Center)\n\
             Checksum (unknown type): abc123\n\
             Installed: 2026-01-01 13:00 CET\n\
             URL: https://example.com/map.zip"
        );
    }

    #[test]
    fn missing_checksum_and_url_show_na() {
        let info = MapInfo {
            source: "Other".to_string(),
            checksum: None,
            checksum_kind: None,
            workshop_id: None,
            url: Some("  ".to_string()),
            ..workshop_info()
        };
        let out = MapInfoCommand::format_info(&info);
        assert!(out.contains("\nChecksum: n/a\n"));
        assert!(out.ends_with("\nURL: n/a"));
    }
}
