// SPDX-License-Identifier: GPL-3.0-only

use crate::steam_bot::bot::SteamBot;
use crate::steam_bot::commands::CommandRegistry;
use crate::steam_bot::messaging::MessageSender;
use SC_Sub_Poster::EnhancedGroupChatMessage;
use std::error::Error;
use std::sync::Arc;

/// Starts the message listener task that listens for incoming Steam chat messages
/// and processes commands when the bot is mentioned.
/// 
/// # Arguments
/// * `bot` - Reference to the SteamBot instance
/// 
/// # Returns
/// A JoinHandle for the listener task
pub fn start_message_listener(bot: Arc<SteamBot>) -> tokio::task::JoinHandle<()> {
    tokio::spawn(async move {
        // Config is accessed through registry, so we don't need to pass it
        if let Err(e) = run_message_listener(bot).await {
            eprintln!("Message listener error: {}", e);
        }
    })
}

/// Main message listener loop
async fn run_message_listener(bot: Arc<SteamBot>) -> Result<(), Box<dyn Error + Send + Sync>> {
    // Get bot Steam ID
    let bot_steam_id_u64 = match bot.get_bot_steam_id().await {
        Some(id) => id,
        None => {
            eprintln!("Warning: Bot Steam ID not available, message listener cannot detect mentions");
            return Ok(());
        }
    };

    println!("Message listener started, waiting for messages...");

    // Listen for group messages - get chat client inside the closure
    loop {
        let bot_for_loop = bot.clone();
        let bot_steam_id_u64_clone = bot_steam_id_u64;
        
        // Get connection to clone chat client, then drop lock before awaiting
        let should_continue = {
            // Get connection to create a new ChatRoomClient - lock is dropped before await
            let connection_opt = {
                let session_guard = bot_for_loop.session.lock().await;
                match session_guard.as_ref() {
                    Some(session) => {
                        // Clone the connection to create a new ChatRoomClient
                        Some(session.chat().connection().clone())
                    }
                    None => None,
                }
            }; // Lock is dropped here - important!
            
            match connection_opt {
                Some(connection) => {
                    // Create a new ChatRoomClient from the cloned connection
                    // This allows us to use it without holding the session lock
                    use SC_Sub_Poster::ChatRoomClient;
                    let chat_client = ChatRoomClient::new(connection);
                    
                    // Listen for group messages - lock is already dropped, so no deadlock
                    match chat_client.listen_for_group_messages(move |message: EnhancedGroupChatMessage| {
                        let bot_steam_id_u64 = bot_steam_id_u64_clone;

                        // Process the message
                        if let Some(response) = process_message(&message, bot_steam_id_u64) {
                            // Send response asynchronously to the same chat room using the global bot instance
                            let chat_group_id = message.chat_group_id;
                            let chat_id = message.chat_id;
                            let response_clone = response.clone();
                            
                            // Use Handle::current() to ensure we can spawn in the callback context
                            let handle = tokio::runtime::Handle::try_current();
                            match handle {
                                Ok(handle) => {
                                    handle.spawn(async move {
                                        if let Err(e) = MessageSender::send_to_chat_global(&response_clone, chat_group_id, chat_id).await {
                                            eprintln!("Failed to send command response: {}", e);
                                        }
                                    });
                                }
                                Err(e) => {
                                    eprintln!("Failed to get runtime handle: {}", e);
                                    eprintln!("Cannot spawn task to send response");
                                }
                            }
                        }
                    }).await {
                        Ok(()) => {
                            // Listener completed normally (shouldn't happen, but handle it)
                            eprintln!("Message listener completed unexpectedly");
                            false
                        }
                        Err(e) => {
                            // Convert error to string immediately
                            let error_msg = format!("{}", e);
                            eprintln!("Message listener error: {}, retrying...", error_msg);
                            true // Continue loop
                        }
                    }
                }
                None => {
                    eprintln!("Warning: Steam session not available, message listener cannot start");
                    return Ok(());
                }
            }
        };
        
        if !should_continue {
            break;
        }
        
        // Wait a bit before retrying
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
    }

    Ok(())
}

/// Processes an incoming message to check if the bot is mentioned and extract commands
/// 
/// # Arguments
/// * `message` - The incoming message
/// * `bot_steam_id_u64` - The bot's Steam ID as u64
/// 
/// # Returns
/// * `Some(String)` - Command response if bot is mentioned and command is found
/// * `None` - If bot is not mentioned or no command found
fn process_message(message: &EnhancedGroupChatMessage, bot_steam_id_u64: u64) -> Option<String> {
    // Check if bot is mentioned via preprocessed mentions
    let is_mentioned_preprocessed = if let Some(ref mentions) = message.preprocessed.mentions {
        mentions.mention_steamids.iter()
            .any(|m| {
                let mentioned_id: u64 = m.as_inner().into();
                mentioned_id == bot_steam_id_u64
            })
    } else {
        false
    };
    
    // Also check BBCode mentions in the message text as fallback
    // Steam chat uses [mention=ACCOUNT_ID]@NAME[/mention] format
    // Account ID is the lower 32 bits of the Steam ID
    let bot_account_id = (bot_steam_id_u64 & 0xFFFFFFFF) as u32;
    
    // Check for BBCode mention format: [mention=ACCOUNT_ID]@NAME[/mention]
    let is_mentioned_bbcode = message.message.contains(&format!("[mention={}]", bot_account_id));
    
    let is_mentioned = is_mentioned_preprocessed || is_mentioned_bbcode;

    if !is_mentioned {
        return None;
    }

    // Extract command from message
    let command_text = extract_command(&message.message);
    
    if command_text.is_empty() {
        return None;
    }

    // Parse command and arguments
    let (command, args) = parse_command(&command_text);
    
    // Handle command
    let registry = CommandRegistry::new();
    registry.handle(&command, &args, message)
}

/// Extracts command text from message (text after "!")
/// 
/// # Arguments
/// * `message` - The message text
/// 
/// # Returns
/// The command text (e.g., "test" from "!test" or "test arg1" from "!test arg1")
fn extract_command(message: &str) -> String {
    // Find the first "!" that's followed by a letter or number
    if let Some(exclamation_pos) = message.find('!') {
        let after_exclamation = &message[exclamation_pos + 1..];
        // Trim whitespace and take everything up to the next space or end of string
        after_exclamation.trim_start().to_string()
    } else {
        String::new()
    }
}

/// Parses command and arguments from command text
/// 
/// # Arguments
/// * `command_text` - The command text (e.g., "test arg1 arg2")
/// 
/// # Returns
/// A tuple of (command, args) where command is lowercase
fn parse_command(command_text: &str) -> (String, String) {
    let parts: Vec<&str> = command_text.splitn(2, char::is_whitespace).collect();
    let command = parts[0].to_lowercase();
    let args = parts.get(1).map(|s| s.trim().to_string()).unwrap_or_default();
    (command, args)
}


