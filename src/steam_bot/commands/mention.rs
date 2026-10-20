// SPDX-License-Identifier: GPL-3.0-only

//! Shared helper for mentioning Steam users in bot replies.

use SC_Sub_Poster::preprocessing_helpers::mention_account_id;
use steam_rs::{steam_id::SteamId, Steam};

/// Upper bound for the persona-name lookup, so command replies stay snappy.
const NAME_LOOKUP_TIMEOUT_MS: u64 = 800;

/// Resolves the user's persona name via the Steam Web API and builds a Steam chat mention
/// (`[mention=<account id>]@<name>[/mention]`, name BBCode-escaped).
///
/// `context` labels the log lines (e.g. "plan caller"). Returns `None` when the API key is
/// empty or the lookup fails / times out; callers decide whether to warn about the key.
pub(crate) async fn user_mention(steam_id: u64, api_key: &str, context: &str) -> Option<String> {
    if api_key.trim().is_empty() {
        return None;
    }

    let steam = Steam::new(api_key);
    let steam_id = SteamId::new(steam_id);
    let name = match tokio::time::timeout(
        tokio::time::Duration::from_millis(NAME_LOOKUP_TIMEOUT_MS),
        steam.get_player_summaries(vec![steam_id]),
    )
    .await
    {
        Ok(Ok(response)) => response.first().map(|player| player.persona_name.clone()),
        Ok(Err(e)) => {
            eprintln!("Failed to resolve {} name for mention: {}", context, e);
            None
        }
        Err(_) => {
            eprintln!("Timed out resolving {} name for mention", context);
            None
        }
    }?;

    Some(mention_account_id(steam_id.get_account_id() as u32, &name))
}
