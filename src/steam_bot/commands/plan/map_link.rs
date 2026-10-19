// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use crate::steam_bot::messaging::MessageSender;
use crate::steam_bot::registry;

/// Resolves the chat to post the map-link message to: the dedicated plan map-links chat when
/// configured, otherwise the dedicated plan chat, otherwise the invoking chat.
fn resolve_target_chat(config: &Config, source_chat_group_id: u64, source_chat_id: u64) -> (u64, u64) {
    config
        .effective_plan_map_links_chat()
        .or_else(|| config.effective_plan_chat())
        .unwrap_or((source_chat_group_id, source_chat_id))
}

/// Looks up `map_name` in the local maps registry and posts its link (or a "not found" note)
/// as a standalone chat message. Target chat priority: dedicated plan map-links chat, then
/// dedicated plan chat, then the invoking chat.
#[cfg(feature = "maps_bridge")]
pub(super) async fn post_map_link(map_name: &str, source_chat_group_id: u64, source_chat_id: u64) {
    let config = registry::config();
    let (chat_group_id, chat_id) = resolve_target_chat(&config, source_chat_group_id, source_chat_id);

    let base_path = match crate::config::exe_dir() {
        Ok(dir) => dir,
        Err(e) => {
            eprintln!("Plan map link: failed to resolve exe dir: {}", e);
            return;
        }
    };

    let message = match crate::maps_bridge::find_installed_map_link(&base_path, &config, map_name) {
        Some(found) => match found.url {
            Some(url) => format!("{} link ({}): {}", found.map_name, found.source, url),
            None => format!("{} ({}) has no download link available.", found.map_name, found.source),
        },
        None => format!("No installed map found matching \"{}\".", map_name),
    };

    if let Err(e) = MessageSender::send_to_chat_global(&message, chat_group_id, chat_id).await {
        eprintln!("Plan map link: failed to send message: {}", e);
    }
}

#[cfg(not(feature = "maps_bridge"))]
pub(super) async fn post_map_link(_map_name: &str, source_chat_group_id: u64, source_chat_id: u64) {
    let config = registry::config();
    let (chat_group_id, chat_id) = resolve_target_chat(&config, source_chat_group_id, source_chat_id);
    let message = "Map link lookup is not available on this server.";
    if let Err(e) = MessageSender::send_to_chat_global(message, chat_group_id, chat_id).await {
        eprintln!("Plan map link: failed to send message: {}", e);
    }
}

#[cfg(test)]
mod tests {
    use super::resolve_target_chat;
    use crate::config::Config;

    fn config_with(toml_extra: &str) -> Config {
        Config::from_toml_str(&format!(
            r#"
frontend_admins = []
[steam.chat]
group_id = 1234
chat_id = 5678
{}
"#,
            toml_extra
        ))
        .expect("config")
    }

    #[test]
    fn resolve_target_chat_prefers_dedicated_map_links_chat() {
        let config = config_with(
            "plan_chat_id = 4321\ndedicated_plan_chat = true\nplan_map_links_chat_id = 9999\ndedicated_plan_map_links_chat = true",
        );
        assert_eq!(resolve_target_chat(&config, 1, 2), (1234, 9999));
    }

    #[test]
    fn resolve_target_chat_falls_back_to_plan_chat_when_map_links_chat_unset() {
        let config = config_with("plan_chat_id = 4321\ndedicated_plan_chat = true");
        assert_eq!(resolve_target_chat(&config, 1, 2), (1234, 4321));
    }

    #[test]
    fn resolve_target_chat_falls_back_to_source_chat_when_neither_configured() {
        let config = config_with("");
        assert_eq!(resolve_target_chat(&config, 1, 2), (1, 2));
    }
}
