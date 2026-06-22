// SPDX-License-Identifier: GPL-3.0-only

use crate::steam_bot::bot::SteamBot;
use crate::steam_bot::registry;
use colored::Colorize;

/// Lists all Steam chat groups and their chats using the global SteamBot instance.
pub async fn list_groups_and_chats() -> Result<(), String> {
    let bot = registry::bot().ok_or_else(|| "SteamBot is not running.".to_string())?;
    list_groups_and_chats_for_bot(&bot).await
}

pub(crate) async fn list_groups_and_chats_for_bot(bot: &SteamBot) -> Result<(), String> {
    let session_guard = bot.session.lock().await;
    let session = session_guard
        .as_ref()
        .ok_or_else(|| "SteamBot is not logged in yet.".to_string())?;

    let groups = session
        .chat()
        .get_my_chat_groups()
        .await
        .map_err(|e| format!("Failed to get chat groups: {:?}", e))?;

    if groups.is_empty() {
        println!("No chat groups found.");
        return Ok(());
    }

    println!("Found {} group(s):\n", groups.len());

    for (i, group) in groups.iter().enumerate() {
        let display_name = if group.chat_group_name.is_empty() {
            "(unnamed)"
        } else {
            group.chat_group_name.as_str()
        };

        println!("{}. {}", i + 1, display_name.bold());
        println!("   Group ID: {}", group.chat_group_id.to_string().bold());
        println!("   Chats:");

        for chat in &group.chats {
            let chat_name = if chat.chat_name.is_empty() {
                "(unnamed)"
            } else {
                chat.chat_name.as_str()
            };
            println!("     - {} (Chat ID: {})", chat_name, chat.chat_id);
        }
        println!();
    }

    Ok(())
}
