// SPDX-License-Identifier: GPL-3.0-only

mod parse;

use super::{CommandContext, CommandError, CommandHandler, CommandInfo, CommandMetadata};
use crate::steam_bot::mute;
use crate::steam_bot::registry;
use async_trait::async_trait;
use parse::{format_duration_minutes, format_remaining_seconds, parse_mute_duration};
use steam_rs::{steam_id::SteamId, Steam};

const STEAM_ID64_BASE: u64 = 76561197960265728;
const MUTE_USAGE: &str =
    "!mute <user mention> [minutes] | !mute <user> h [H | H:MM] | !mute <user> d [days]";
const UNMUTE_USAGE: &str = "!unmute <user mention>";
const LSMUTE_USAGE: &str = "!lsmute";

fn require_admin(sender_id: u64) -> Result<(), CommandError> {
    if registry::config().is_steambot_admin(sender_id) {
        Ok(())
    } else {
        Err(CommandError::Unauthorized(
            "You are not authorized to use this command.".to_string(),
        ))
    }
}

fn strip_mention_bbcode(args: &str) -> String {
    let mut result = String::new();
    let mut rest = args;
    while let Some(start) = rest.find("[mention=") {
        result.push_str(&rest[..start]);
        if let Some(end_rel) = rest[start..].find("[/mention]") {
            let end = start + end_rel + "[/mention]".len();
            rest = &rest[end..];
        } else {
            result.push_str(&rest[start..]);
            return result.trim().to_string();
        }
    }
    result.push_str(rest);
    result.trim().to_string()
}

fn mention_account_id_from_args(args: &str) -> Option<u32> {
    let start = args.find("[mention=")?;
    let after = &args[start + "[mention=".len()..];
    let end = after.find(']')?;
    after[..end].parse().ok()
}

fn steam_id64_from_account_id(account_id: u32) -> u64 {
    STEAM_ID64_BASE + u64::from(account_id)
}

async fn resolve_target_steam_id(ctx: &CommandContext<'_>) -> Result<u64, CommandError> {
    let bot_steam_id = if let Some(bot) = registry::bot() {
        bot.get_bot_steam_id().await.unwrap_or(0)
    } else {
        0
    };

    if let Some(mentions) = &ctx.message.preprocessed.mentions {
        for mention in &mentions.mention_steamids {
            let mentioned_id: u64 = mention.as_inner().into();
            if mentioned_id != bot_steam_id {
                return Ok(mentioned_id);
            }
        }
    }

    if let Some(account_id) = mention_account_id_from_args(ctx.args) {
        return Ok(steam_id64_from_account_id(account_id));
    }

    let stripped = strip_mention_bbcode(ctx.args);
    for token in stripped.split_whitespace() {
        if token.len() == 17 && token.chars().all(|ch| ch.is_ascii_digit()) {
            if let Ok(steam_id) = token.parse::<u64>() {
                return Ok(steam_id);
            }
        }
    }

    Err(CommandError::InvalidArguments(
        "Could not resolve target user. Mention a user or provide a SteamID64.".to_string(),
    ))
}

async fn resolve_display_name(steam_id: u64) -> String {
    let config = registry::config();
    if config.steam_web_api_key.trim().is_empty() {
        return steam_id.to_string();
    }

    let steam = Steam::new(&config.steam_web_api_key);
    let steam_id_obj = SteamId::new(steam_id);
    match tokio::time::timeout(
        tokio::time::Duration::from_millis(800),
        steam.get_player_summaries(vec![steam_id_obj]),
    )
    .await
    {
        Ok(Ok(response)) => response
            .first()
            .map(|player| player.persona_name.clone())
            .unwrap_or_else(|| steam_id.to_string()),
        _ => steam_id.to_string(),
    }
}

fn duration_tokens(args: &str) -> Vec<String> {
    strip_mention_bbcode(args)
        .split_whitespace()
        .map(str::to_string)
        .collect()
}

struct MuteCommand;

#[async_trait]
impl CommandHandler for MuteCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        require_admin(ctx.sender_id)?;
        let target_id = resolve_target_steam_id(ctx).await?;
        let token_strings = duration_tokens(ctx.args);
        let tokens: Vec<&str> = token_strings.iter().map(String::as_str).collect();
        let mut duration_minutes = parse_mute_duration(&tokens)
            .map_err(CommandError::InvalidArguments)?;

        let config = registry::config();
        let max_minutes = config.steambot_mute_max_minutes;
        let capped = duration_minutes > max_minutes;
        if capped {
            duration_minutes = max_minutes;
        }

        let _unmute_at = mute::add_mute(target_id, duration_minutes).await;
        let display_name = resolve_display_name(target_id).await;
        let duration_text = format_duration_minutes(duration_minutes);
        if capped {
            Ok(format!(
                "Muted {} for {} (capped to max {}).",
                display_name, duration_text, format_duration_minutes(max_minutes)
            ))
        } else {
            Ok(format!("Muted {} for {}.", display_name, duration_text))
        }
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "mute",
            aliases: &[],
            description: "Mutes a user; their messages are auto-deleted until the mute expires.",
            usage: Some(MUTE_USAGE),
        };
        &METADATA
    }
}

struct UnmuteCommand;

#[async_trait]
impl CommandHandler for UnmuteCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        require_admin(ctx.sender_id)?;
        let target_id = resolve_target_steam_id(ctx).await?;
        let display_name = resolve_display_name(target_id).await;
        if mute::remove_mute(target_id).await {
            Ok(format!("Unmuted {}.", display_name))
        } else {
            Ok(format!("{} was not muted.", display_name))
        }
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "unmute",
            aliases: &[],
            description: "Removes an active mute from a user.",
            usage: Some(UNMUTE_USAGE),
        };
        &METADATA
    }
}

struct LsmuteCommand;

#[async_trait]
impl CommandHandler for LsmuteCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        require_admin(ctx.sender_id)?;
        let _ = ctx.args;
        let mutes = mute::list_mutes();
        if mutes.is_empty() {
            return Ok("No users are muted.".to_string());
        }

        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_secs() as i64)
            .unwrap_or(0);

        let mut lines = Vec::with_capacity(mutes.len());
        for (steam_id, unmute_at) in mutes {
            let display_name = resolve_display_name(steam_id).await;
            let remaining = format_remaining_seconds(unmute_at - now);
            lines.push(format!("{} - {} left", display_name, remaining));
        }

        Ok(lines.join("\n"))
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "lsmute",
            aliases: &[],
            description: "Lists currently muted users and remaining mute time.",
            usage: Some(LSMUTE_USAGE),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "mute",
        &[],
        "Mutes a user; their messages are auto-deleted until the mute expires.",
        Some(MUTE_USAGE),
        || Box::new(MuteCommand)
    )
}

inventory::submit! {
    CommandInfo::new(
        "unmute",
        &[],
        "Removes an active mute from a user.",
        Some(UNMUTE_USAGE),
        || Box::new(UnmuteCommand)
    )
}

inventory::submit! {
    CommandInfo::new(
        "lsmute",
        &[],
        "Lists currently muted users and remaining mute time.",
        Some(LSMUTE_USAGE),
        || Box::new(LsmuteCommand)
    )
}
