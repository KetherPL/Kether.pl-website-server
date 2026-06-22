// SPDX-License-Identifier: GPL-3.0-only

use crate::steam_bot::registry;
use steam_rs::{steam_id::SteamId, Steam};

pub(super) async fn plan_actor_mention(actor: Option<u64>) -> Option<String> {
    let Some(actor_steam_id_u64) = actor else {
        return None;
    };

    let config = registry::config();
    if !config.plan_mention_user {
        return None;
    }
    if config.steam_web_api_key.trim().is_empty() {
        eprintln!(
            "Warning: steam.chat.plan_mention_user is enabled but steam.web_api_key is empty. Skipping user mention."
        );
        return None;
    }

    let steam = Steam::new(&config.steam_web_api_key);
    let actor_steam_id = SteamId::new(actor_steam_id_u64);
    let caller_name = match tokio::time::timeout(
        tokio::time::Duration::from_millis(800),
        steam.get_player_summaries(vec![actor_steam_id]),
    )
    .await
    {
        Ok(Ok(response)) => response.first().map(|player| player.persona_name.clone()),
        Ok(Err(e)) => {
            eprintln!("Failed to resolve plan caller name for mention: {}", e);
            None
        }
        Err(_) => {
            eprintln!("Timed out resolving plan caller name for mention");
            None
        }
    }?;

    let account_id = actor_steam_id.get_account_id();
    Some(format!("[mention={}]@{}[/mention]", account_id, caller_name))
}
