// SPDX-License-Identifier: GPL-3.0-only

use super::{CommandContext, CommandError, CommandHandler, CommandInfo, CommandMetadata};
use async_trait::async_trait;

const MAPS_USAGE: &str = "!maps [c|s|w|o|S] (c = L4D2Center, s = SirPlease, w = Workshop, o = Other, S = Suggestions; none = all)";
const MAPS_DESCRIPTION: &str = "Lists installed maps grouped by source (optionally only the selected source(s)).";

/// Map source sections shown by `!maps`, in display order.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum MapsSource {
    L4D2Center,
    SirPlease,
    Workshop,
    Other,
    Suggestions,
}

impl MapsSource {
    const ALL: [MapsSource; 5] = [
        MapsSource::L4D2Center,
        MapsSource::SirPlease,
        MapsSource::Workshop,
        MapsSource::Other,
        MapsSource::Suggestions,
    ];

    /// Section label; for installed sources it matches the maps bridge source label.
    fn label(self) -> &'static str {
        match self {
            MapsSource::L4D2Center => "L4D2Center",
            MapsSource::SirPlease => "SirPlease",
            MapsSource::Workshop => "Workshop",
            MapsSource::Other => "Other",
            MapsSource::Suggestions => "Suggestions",
        }
    }

    /// Parses a single argument. One-letter flags are case-sensitive (`s` = SirPlease,
    /// `S` = Suggestions); full source names are accepted case-insensitively.
    fn from_arg(arg: &str) -> Option<Self> {
        match arg {
            "c" => return Some(MapsSource::L4D2Center),
            "s" => return Some(MapsSource::SirPlease),
            "w" => return Some(MapsSource::Workshop),
            "o" => return Some(MapsSource::Other),
            "S" => return Some(MapsSource::Suggestions),
            _ => {}
        }
        Self::ALL
            .into_iter()
            .find(|source| source.label().eq_ignore_ascii_case(arg))
    }
}

/// Lists installed maps grouped by source.
struct MapsCommand;

impl MapsCommand {
    /// Returns the requested sources in display order, or `None` when no filter was given.
    fn parse_sources(args: &str) -> Result<Option<Vec<MapsSource>>, CommandError> {
        let mut selected = Vec::new();
        for arg in args.split_whitespace() {
            let source = MapsSource::from_arg(arg).ok_or_else(|| {
                CommandError::InvalidArguments(format!(
                    "Unknown map source \"{}\". Usage: {}",
                    arg, MAPS_USAGE
                ))
            })?;
            if !selected.contains(&source) {
                selected.push(source);
            }
        }

        if selected.is_empty() {
            return Ok(None);
        }
        Ok(Some(
            MapsSource::ALL
                .into_iter()
                .filter(|source| selected.contains(source))
                .collect(),
        ))
    }

    /// Formats the listing. `installed` holds `(source label, map name)` pairs.
    /// Without a filter, empty sections are skipped; with one, they are shown as "(none)".
    fn format_listing(
        installed: &[(String, String)],
        suggestions: &[String],
        filter: Option<&[MapsSource]>,
    ) -> String {
        let sources: &[MapsSource] = filter.unwrap_or(&MapsSource::ALL);
        let mut sections = Vec::new();

        for &source in sources {
            let mut names: Vec<&str> = match source {
                MapsSource::Suggestions => suggestions.iter().map(String::as_str).collect(),
                _ => installed
                    .iter()
                    .filter(|(label, _)| label == source.label())
                    .map(|(_, name)| name.as_str())
                    .collect(),
            };

            if names.is_empty() && filter.is_none() {
                continue;
            }

            names.sort_by(|a, b| a.to_lowercase().cmp(&b.to_lowercase()).then_with(|| a.cmp(b)));

            let mut section = format!("[{}]", source.label());
            if names.is_empty() {
                section.push_str("\n(none)");
            }
            for (index, name) in names.iter().enumerate() {
                section.push_str(&format!("\n{}. {}", index + 1, name));
            }
            sections.push(section);
        }

        if sections.is_empty() {
            return "No installed maps found.".to_string();
        }
        format!("Installed maps:\n\n{}", sections.join("\n\n"))
    }
}

#[async_trait]
impl CommandHandler for MapsCommand {
    #[cfg(feature = "maps_bridge")]
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        let filter = Self::parse_sources(ctx.args)?;

        let base_path = crate::config::exe_dir().map_err(|e| {
            eprintln!("!maps: failed to resolve exe dir: {}", e);
            CommandError::ServerError("Failed to load the maps list.".to_string())
        })?;
        let config = crate::steam_bot::registry::config();
        let listing = crate::maps_bridge::list_maps_for_chat(&base_path, &config).map_err(|e| {
            eprintln!("!maps: {}", e);
            CommandError::ServerError("Failed to load the maps list.".to_string())
        })?;

        Ok(Self::format_listing(
            &listing.installed,
            &listing.suggestions,
            filter.as_deref(),
        ))
    }

    #[cfg(not(feature = "maps_bridge"))]
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        Self::parse_sources(ctx.args)?;
        Ok("Maps list is not available on this server.".to_string())
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "maps",
            aliases: &[],
            description: MAPS_DESCRIPTION,
            usage: Some(MAPS_USAGE),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "maps",
        &[],
        MAPS_DESCRIPTION,
        Some(MAPS_USAGE),
        || Box::new(MapsCommand)
    )
}

#[cfg(test)]
mod tests {
    use super::{MapsCommand, MapsSource};
    use crate::steam_bot::commands::CommandError;

    fn installed() -> Vec<(String, String)> {
        [
            ("Workshop", "some nice map"),
            ("L4D2Center", "yet another map"),
            ("Workshop", "An old but loved map"),
            ("L4D2Center", "another map"),
            ("Workshop", "a different map"),
            ("L4D2Center", "Some map"),
        ]
        .into_iter()
        .map(|(source, name)| (source.to_string(), name.to_string()))
        .collect()
    }

    #[test]
    fn parse_sources_empty_means_all() {
        assert_eq!(MapsCommand::parse_sources("").unwrap(), None);
        assert_eq!(MapsCommand::parse_sources("   ").unwrap(), None);
    }

    #[test]
    fn parse_sources_letter_flags_are_case_sensitive() {
        assert_eq!(
            MapsCommand::parse_sources("s").unwrap(),
            Some(vec![MapsSource::SirPlease])
        );
        assert_eq!(
            MapsCommand::parse_sources("S").unwrap(),
            Some(vec![MapsSource::Suggestions])
        );
        assert!(matches!(
            MapsCommand::parse_sources("W"),
            Err(CommandError::InvalidArguments(_))
        ));
    }

    #[test]
    fn parse_sources_accepts_full_names_and_orders_dedups() {
        assert_eq!(
            MapsCommand::parse_sources("S workshop c w").unwrap(),
            Some(vec![
                MapsSource::L4D2Center,
                MapsSource::Workshop,
                MapsSource::Suggestions
            ])
        );
    }

    #[test]
    fn parse_sources_rejects_unknown() {
        match MapsCommand::parse_sources("x") {
            Err(CommandError::InvalidArguments(msg)) => {
                assert!(msg.contains("\"x\""));
                assert!(msg.contains("!maps"));
            }
            other => panic!("expected InvalidArguments, got {:?}", other.map(|_| ())),
        }
    }

    #[test]
    fn format_all_groups_sorts_and_skips_empty_sections() {
        let suggestions = vec!["Zzz".to_string(), "Aaa".to_string()];
        let out = MapsCommand::format_listing(&installed(), &suggestions, None);
        assert_eq!(
            out,
            "Installed maps:\n\n\
             [L4D2Center]\n1. another map\n2. Some map\n3. yet another map\n\n\
             [Workshop]\n1. a different map\n2. An old but loved map\n3. some nice map\n\n\
             [Suggestions]\n1. Aaa\n2. Zzz"
        );
    }

    #[test]
    fn format_filtered_shows_only_selected_and_marks_empty() {
        let filter = [MapsSource::SirPlease, MapsSource::Workshop];
        let out = MapsCommand::format_listing(&installed(), &[], Some(&filter));
        assert_eq!(
            out,
            "Installed maps:\n\n\
             [SirPlease]\n(none)\n\n\
             [Workshop]\n1. a different map\n2. An old but loved map\n3. some nice map"
        );
    }

    #[test]
    fn format_nothing_installed() {
        assert_eq!(
            MapsCommand::format_listing(&[], &[], None),
            "No installed maps found."
        );
    }
}
