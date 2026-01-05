// SPDX-License-Identifier: GPL-3.0-only

use crate::steam_bot::bot::SteamBot;
use crate::steam_bot::commands::CommandRegistry;
use crate::steam_bot::connection::ConnectionManager;
use crate::steam_bot::messaging::MessageSender;
use crate::steam_bot::registry;
use crate::steam_bot::utils::is_connection_error;
use SC_Sub_Poster::EnhancedGroupChatMessage;
use std::error::Error;
use std::sync::Arc;
use tokio::time::Duration;

/// Retry delay when the listener encounters an error
const LISTENER_RETRY_DELAY_SECS: u64 = 5;

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
    let bot_steam_id = get_bot_steam_id(&bot).await?;
    println!("Message listener started, waiting for messages...");

    loop {
        match create_chat_client(&bot).await {
            Some(chat_client) => {
                match listen_for_messages(chat_client, bot_steam_id).await {
                    Ok(()) => {
                        eprintln!("Message listener completed unexpectedly");
                        break;
                    }
                    Err(e) => {
                        // Convert error to string immediately to ensure Send
                        let error_msg = format!("{}", e);
                        eprintln!("Message listener error: {}", error_msg);
                        
                        // Check if this is a connection error and attempt recovery
                        if is_connection_error(&error_msg) {
                            eprintln!("Connection error detected, attempting reconnection...");
                            if let Some(config) = registry::config_opt() {
                                match ConnectionManager::reconnect(&bot, config).await {
                                    Ok(()) => {
                                        println!("Successfully reconnected, retrying listener...");
                                        // Wait a bit before retrying to ensure connection is stable
                                        wait_before_retry().await;
                                    }
                                    Err(reconnect_err) => {
                                        eprintln!("Failed to reconnect: {}, retrying listener anyway...", reconnect_err);
                                        wait_before_retry().await;
                                    }
                                }
                            } else {
                                eprintln!("Config not available, cannot reconnect. Retrying listener...");
                                wait_before_retry().await;
                            }
                        } else {
                            // Not a connection error, just wait and retry
                            eprintln!("Non-connection error, retrying...");
                            wait_before_retry().await;
                        }
                    }
                }
            }
            None => {
                eprintln!("Warning: Steam session not available, message listener cannot start");
                // If session is not available, try to reconnect if config is available
                if let Some(config) = registry::config_opt() {
                    eprintln!("Attempting to reconnect...");
                    if let Err(e) = ConnectionManager::reconnect(&bot, config).await {
                        eprintln!("Failed to reconnect: {}", e);
                    }
                    wait_before_retry().await;
                } else {
                    return Ok(());
                }
            }
        }
    }

    Ok(())
}

/// Gets the bot's Steam ID, returning an error if unavailable
async fn get_bot_steam_id(bot: &Arc<SteamBot>) -> Result<u64, Box<dyn Error + Send + Sync>> {
    bot.get_bot_steam_id()
        .await
        .ok_or_else(|| "Bot Steam ID not available, message listener cannot detect mentions".into())
}

/// Creates a new ChatRoomClient from the bot's session connection
/// 
/// Returns None if the session is not available. The connection is cloned
/// so the lock can be dropped before awaiting the listener.
async fn create_chat_client(bot: &Arc<SteamBot>) -> Option<SC_Sub_Poster::ChatRoomClient> {
    let connection = {
        let session_guard = bot.session.lock().await;
        session_guard.as_ref()?.chat().connection().clone()
    }; // Lock is dropped here - important to avoid deadlock
    
    Some(SC_Sub_Poster::ChatRoomClient::new(connection))
}

/// Listens for incoming messages and processes commands
async fn listen_for_messages(
    chat_client: SC_Sub_Poster::ChatRoomClient,
    bot_steam_id: u64,
) -> Result<(), String> {
    chat_client.listen_for_group_messages(move |message: EnhancedGroupChatMessage| {
        if let Some(response) = process_message(&message, bot_steam_id) {
            send_command_response(&message, &response);
        }
    }).await.map_err(|e| format!("{}", e))
}

/// Sends a command response to the same chat room as the incoming message
fn send_command_response(message: &EnhancedGroupChatMessage, response: &str) {
    let chat_group_id = message.chat_group_id;
    let chat_id = message.chat_id;
    let response = response.to_string();
    
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            handle.spawn(async move {
                if let Err(e) = MessageSender::send_to_chat_global(&response, chat_group_id, chat_id).await {
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

/// Waits before retrying the listener after an error
async fn wait_before_retry() {
    tokio::time::sleep(Duration::from_secs(LISTENER_RETRY_DELAY_SECS)).await;
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

    // Check configuration for commands without mention
    let config = registry::config();
    let allow_without_mention = config.steam_bot_commands_without_mention;

    if !is_mentioned && !allow_without_mention {
        return None;
    }

    // Extract command from message
    // If not mentioned, we only want to process messages that start with "!" (ignoring leading whitespace)
    let command_text = if !is_mentioned && allow_without_mention {
        let trimmed = message.message.trim_start();
        if trimmed.starts_with('!') {
            // It starts with '!', extract the command part (everything after the first '!')
            let exclamation_pos = trimmed.find('!').unwrap();
            let after_exclamation = &trimmed[exclamation_pos + 1..];
            after_exclamation.trim_start().to_string()
        } else {
            return None;
        }
    } else {
        extract_command(&message.message)
    };
    
    if command_text.is_empty() {
        return None;
    }

    // Parse command and arguments
    let (command, args) = parse_command(&command_text);
    
    // Check if command exists
    let command_lower = command.to_lowercase();
    
    // We want to handle all commands asynchronously to not block the listener
    let chat_group_id = message.chat_group_id;
    let chat_id = message.chat_id;
    let message_cloned = message.clone();
    let args_cloned = args.clone();
    let command_cloned = command.clone();

    // Spawn async task to handle the command and send response
    match tokio::runtime::Handle::try_current() {
        Ok(handle) => {
            let needs_placeholder = command_lower == "status" || command_lower == "s";
            
            handle.spawn(async move {
                // Send placeholder message only for commands that need it (like !status)
                let placeholder_preprocessed = if needs_placeholder {
                    match MessageSender::send_to_chat_global_with_preprocessed("Querying server...", chat_group_id, chat_id).await {
                        Ok(preprocessed) => Some(preprocessed),
                        Err(e) => {
                            eprintln!("Failed to send placeholder message: {}", e);
                            None
                        }
                    }
                } else {
                    None
                };
                
                // Execute the command via registry
                let registry = CommandRegistry::new();
                let response = match registry.handle(&command_cloned, &args_cloned, &message_cloned).await {
                    Ok(Some(res)) => res,
                    Ok(None) => return, // Command not found, should probably not happen if we checked before
                    Err(e) => e.to_string(), // Use the user-friendly error message
                };
                
                // Send the actual response
                if let Err(e) = MessageSender::send_to_chat_global(&response, chat_group_id, chat_id).await {
                    eprintln!("Failed to send command response: {}", e);
                }
                
                // Delete the placeholder message if we have its PreprocessedMessage
                if let Some(preprocessed) = placeholder_preprocessed {
                    if let Err(e) = MessageSender::delete_message_global(chat_group_id, chat_id, preprocessed).await {
                        eprintln!("Failed to delete placeholder message: {}", e);
                    }
                }
            });
        }
        Err(e) => {
            eprintln!("Failed to get runtime handle for command execution: {}", e);
        }
    }

    None
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


