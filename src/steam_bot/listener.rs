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

/// Health check interval for proactive connection monitoring
/// Matches the interval used in ConnectionManager::ensure_healthy()
const HEALTH_CHECK_INTERVAL_SECS: u64 = 300; // 5 minutes

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
        // Note: We can't format the error here because it might not be Send
        // The error is already logged inside run_message_listener
        if let Err(_) = run_message_listener(bot).await {
            eprintln!("Message listener task exited with an error (check logs above for details)");
        } else {
            eprintln!("Message listener exited unexpectedly (should run indefinitely)");
        }
    })
}

/// Main message listener loop
async fn run_message_listener(bot: Arc<SteamBot>) -> Result<(), Box<dyn Error + Send + Sync>> {
    let bot_steam_id = get_bot_steam_id(&bot).await?;
    println!("Message listener started, waiting for messages...");

    // Spawn background health check task
    // Use ensure_healthy() which properly handles connection state and recovery
    let bot_for_health_check = bot.clone();
    tokio::spawn(async move {
        let mut health_check_interval = tokio::time::interval(Duration::from_secs(HEALTH_CHECK_INTERVAL_SECS));
        health_check_interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        
        loop {
            // Use catch_unwind or handle errors to prevent task from silently stopping
            health_check_interval.tick().await;
            
            if let Some(config) = registry::config_opt() {
                // Use ensure_healthy() which checks connection state and properly handles recovery
                // This matches what the call-for-sub bot uses and ensures Failed state is handled
                if let Err(e) = ConnectionManager::ensure_healthy(&bot_for_health_check, config, false).await {
                    eprintln!("Listener health check failed: {}", e);
                    // Continue loop even on error - don't let health check task die
                }
            } else {
                // Config not available - log and continue (might be temporary)
                eprintln!("Config not available for health check, will retry on next interval");
            }
        }
    });

    loop {
        // Check connection state before creating client
        let connection_state = bot.get_connection_state().await;
        println!("Listener loop iteration: connection state = {:?}", connection_state);
        match connection_state {
            crate::steam_bot::state::ConnectionState::Reconnecting => {
                // Wait for reconnection to complete (health check task is handling it)
                eprintln!("Connection is reconnecting, waiting...");
                wait_before_retry().await;
                continue;
            }
            crate::steam_bot::state::ConnectionState::Failed => {
                // Use ensure_healthy() to handle reconnection (avoids double reconnection with health check task)
                eprintln!("Connection state is Failed, attempting reconnection...");
                if let Some(config) = registry::config_opt() {
                    // Use ensure_healthy() which has retry logic and proper state management
                    if let Err(e) = ConnectionManager::ensure_healthy(&bot, config, true).await {
                        eprintln!("Failed to recover from Failed state: {}", e);
                        wait_before_retry().await;
                        continue;
                    } else {
                        println!("Successfully recovered from Failed state");
                        wait_before_retry().await; // Brief wait to ensure connection is stable
                    }
                } else {
                    eprintln!("Config not available, cannot reconnect from Failed state");
                    wait_before_retry().await;
                    continue;
                }
            }
            crate::steam_bot::state::ConnectionState::Disconnected | crate::steam_bot::state::ConnectionState::Connecting => {
                // Wait for connection to be established
                eprintln!("Connection is not ready (state: {:?}), waiting...", connection_state);
                wait_before_retry().await;
                continue;
            }
            crate::steam_bot::state::ConnectionState::Connected => {
                // Connection is ready, but validate session before proceeding
                if !validate_session(&bot).await {
                    eprintln!("Connection state is Connected but session is not available, attempting recovery...");
                    if let Some(config) = registry::config_opt() {
                        if let Err(e) = ConnectionManager::ensure_healthy(&bot, config, true).await {
                            eprintln!("Failed to recover session: {}", e);
                        }
                    }
                    wait_before_retry().await;
                    continue;
                }
                
                // Perform a quick health check before creating client
                if let Some(config) = registry::config_opt() {
                    // Convert error to string immediately to ensure Send
                    let health_ok = match ConnectionManager::check_health(&bot).await {
                        Ok(true) => true,
                        Ok(false) => {
                            eprintln!("Health check failed before creating client, attempting recovery...");
                            false
                        }
                        Err(e) => {
                            let error_msg = format!("{}", e);
                            eprintln!("Health check error before creating client: {}, attempting recovery...", error_msg);
                            false
                        }
                    };
                    
                    if !health_ok {
                        if let Err(e) = ConnectionManager::ensure_healthy(&bot, config, false).await {
                            eprintln!("Failed to recover: {}", e);
                        }
                        wait_before_retry().await;
                        continue;
                    }
                }
            }
        }
        
        match create_chat_client(&bot).await {
            Some(chat_client) => {
                // Validate session is still valid after creating client
                if !validate_session(&bot).await {
                    eprintln!("Session became invalid after creating client, attempting recovery...");
                    if let Some(config) = registry::config_opt() {
                        if let Err(e) = ConnectionManager::ensure_healthy(&bot, config, true).await {
                            eprintln!("Failed to recover: {}", e);
                        }
                    }
                    wait_before_retry().await;
                    continue;
                }
                
                // Perform one more health check right before listening
                if let Some(config) = registry::config_opt() {
                    // Convert error to string immediately to ensure Send
                    let health_ok = match ConnectionManager::check_health(&bot).await {
                        Ok(true) => {
                            println!("Created chat client, connection validated, starting to listen for messages...");
                            true
                        }
                        Ok(false) => {
                            eprintln!("Health check failed after creating client, attempting recovery...");
                            false
                        }
                        Err(e) => {
                            let error_msg = format!("{}", e);
                            eprintln!("Health check error after creating client: {}, attempting recovery...", error_msg);
                            false
                        }
                    };
                    
                    if !health_ok {
                        if let Err(e) = ConnectionManager::ensure_healthy(&bot, config, true).await {
                            eprintln!("Failed to recover: {}", e);
                        }
                        wait_before_retry().await;
                        continue;
                    }
                } else {
                    println!("Created chat client, starting to listen for messages...");
                }
                
                // Get the current generation to monitor for changes
                let current_generation = bot.get_generation().await;
                
                match listen_for_messages(chat_client, bot_steam_id, &bot, current_generation).await {
                    Ok(()) => {
                        // listen_for_group_messages returned Ok(()) - this means the stream ended
                        // This can happen when the connection is lost, so we should retry
                        eprintln!("Message listener stream ended (connection may be lost), attempting recovery...");
                        if let Some(config) = registry::config_opt() {
                            // Use ensure_healthy() to handle reconnection with proper state management
                            // This avoids race conditions with the health check task
                            match ConnectionManager::ensure_healthy(&bot, config, true).await {
                                Ok(()) => {
                                    println!("Successfully recovered connection, retrying listener...");
                                    // Wait a bit before retrying to ensure connection is stable
                                    wait_before_retry().await;
                                }
                                Err(reconnect_err) => {
                                    eprintln!("Failed to recover connection: {}, retrying listener anyway...", reconnect_err);
                                    wait_before_retry().await;
                                }
                            }
                        } else {
                            eprintln!("Config not available, cannot recover connection. Retrying listener...");
                            wait_before_retry().await;
                        }
                        // Continue loop to retry - don't exit!
                    }
                    Err(e) => {
                        // Convert error to string immediately to ensure Send
                        let error_msg = format!("{}", e);
                        eprintln!("Message listener error: {}", error_msg);
                        
                        // Check if this is a connection error and attempt recovery
                        if is_connection_error(&error_msg) {
                            eprintln!("Connection error detected, attempting reconnection...");
                            if let Some(config) = registry::config_opt() {
                                // Use ensure_healthy() to handle reconnection with proper state management
                                // This avoids race conditions with the health check task
                                match ConnectionManager::ensure_healthy(&bot, config, true).await {
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
                // Re-check connection state - might be reconnecting already
                let connection_state = bot.get_connection_state().await;
                match connection_state {
                    crate::steam_bot::state::ConnectionState::Reconnecting => {
                        // Health check task is already handling reconnection, just wait
                        eprintln!("Connection is reconnecting, waiting for health check task...");
                        wait_before_retry().await;
                    }
                    _ => {
                        // If session is not available, try to reconnect if config is available
                        if let Some(config) = registry::config_opt() {
                            eprintln!("Attempting to reconnect...");
                            // Use ensure_healthy() to handle reconnection with proper state management
                            if let Err(e) = ConnectionManager::ensure_healthy(&bot, config, true).await {
                                eprintln!("Failed to reconnect: {}", e);
                            }
                            wait_before_retry().await;
                        } else {
                            return Ok(());
                        }
                    }
                }
            }
        }
    }
}

/// Gets the bot's Steam ID, returning an error if unavailable
async fn get_bot_steam_id(bot: &Arc<SteamBot>) -> Result<u64, Box<dyn Error + Send + Sync>> {
    bot.get_bot_steam_id()
        .await
        .ok_or_else(|| "Bot Steam ID not available, message listener cannot detect mentions".into())
}

/// Validates that the session is still valid and usable
/// 
/// Returns true if the session exists and appears to be valid
async fn validate_session(bot: &Arc<SteamBot>) -> bool {
    let session_guard = bot.session.lock().await;
    session_guard.is_some()
}

/// Creates a new ChatRoomClient from the bot's session connection
/// 
/// Returns None if the session is not available. The connection is cloned
/// so the lock can be dropped before awaiting the listener.
/// 
/// Also validates the session before creating the client.
async fn create_chat_client(bot: &Arc<SteamBot>) -> Option<SC_Sub_Poster::ChatRoomClient> {
    // First validate that session exists
    if !validate_session(bot).await {
        eprintln!("Cannot create chat client: session is not available");
        return None;
    }
    
    let connection = {
        let session_guard = bot.session.lock().await;
        session_guard.as_ref()?.chat().connection().clone()
    }; // Lock is dropped here - important to avoid deadlock
    
    // Validate connection was successfully cloned
    Some(SC_Sub_Poster::ChatRoomClient::new(connection))
}

/// Listens for incoming messages and processes commands
/// 
/// Monitors the bot's generation counter and returns an error if it changes,
/// indicating that the underlying connection has been replaced (reconnected).
async fn listen_for_messages(
    chat_client: SC_Sub_Poster::ChatRoomClient,
    bot_steam_id: u64,
    bot: &Arc<SteamBot>,
    initial_generation: u64,
) -> Result<(), String> {
    let listener_future = chat_client.listen_for_group_messages(move |message: EnhancedGroupChatMessage| {
        if let Some(response) = process_message(&message, bot_steam_id) {
            send_command_response(&message, &response);
        }
    });

    let bot_clone = bot.clone();
    let monitor_future = async move {
        loop {
            tokio::time::sleep(Duration::from_secs(30)).await;
            if bot_clone.get_generation().await != initial_generation {
                return Err("Connection generation changed (reconnected remotely)".to_string());
            }
        }
    };

    tokio::select! {
        res = listener_future => res.map_err(|e| format!("{}", e)),
        res = monitor_future => res,
    }
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
            // Safe to skip first char since we verified it starts with '!'
            trimmed[1..].trim_start().to_string()
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
                    match MessageSender::send_to_chat_global_with_preprocessed("Querying servers...", chat_group_id, chat_id).await {
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


