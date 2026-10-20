// SPDX-License-Identifier: GPL-3.0-only

use crate::steam_bot::commands::mention::user_mention;
use crate::steam_bot::registry;

pub(super) async fn poll_actor_mention(actor: Option<u64>) -> Option<String> {
    let actor_steam_id_u64 = actor?;

    let config = registry::config();
    if !config.poll_mention_user {
        return None;
    }
    if config.steam_web_api_key.trim().is_empty() {
        eprintln!(
            "Warning: steam.chat.poll_mention_user is enabled but steam.web_api_key is empty. Skipping user mention."
        );
        return None;
    }

    user_mention(actor_steam_id_u64, &config.steam_web_api_key, "poll caller").await
}
