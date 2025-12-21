// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use crate::steam_bot::bot::SteamBot;
use crate::steam_bot::connection::ConnectionManager;
use crate::steam_bot::registry;
use crate::steam_bot::utils::is_connection_error;
use SC_Sub_Poster::SendGroupMessageParams;

/// Message sender for SteamBot
/// 
/// This module provides functionality for sending messages to Steam group chats
/// with automatic retry and recovery logic.
pub struct MessageSender;

impl MessageSender {
    /// Handles connection errors by attempting reconnection and retry
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `config` - Reference to the Config
    /// * `error` - The connection error that occurred
    /// 
    /// # Returns
    /// * `Ok(())` - If reconnection and retry succeeded
    /// * `Err(Box<dyn std::error::Error>)` - If reconnection failed
    async fn handle_connection_error(
        bot: &SteamBot,
        config: &Config,
        error: Box<dyn std::error::Error + Send + Sync>,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        println!("Message send failed due to connection issue: {}", error);
        println!("Attempting immediate reconnection and retry...");
        
        // Attempt to reconnect and retry once
        ConnectionManager::reconnect(bot, config).await?;
        Ok(())
    }

    /// Sends a message with automatic recovery on connection failures
    /// 
    /// If a connection error occurs, this function automatically attempts
    /// to reconnect and retry sending the message once.
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `message` - The message to send
    /// * `config` - Reference to the Config
    /// 
    /// # Returns
    /// * `Ok(())` - If message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails after retry
    async fn send_with_immediate_recovery(
        bot: &SteamBot,
        message: &str,
        config: &Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match Self::send(bot, message, config).await {
            Ok(()) => Ok(()),
            Err(e) => {
                let error_str = e.to_string().to_lowercase();
                if is_connection_error(&error_str) {
                    Self::handle_connection_error(bot, config, e).await?;
                    // Retry sending the message
                    Self::send(bot, message, config).await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Sends a message to the Steam group chat
    /// 
    /// This function sends a message to the specified Steam group chat using
    /// the chat_group_id and chat_id from the config. The message will be
    /// sent to all members of the group chat.
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `message` - The message to send to the Steam group chat
    /// * `config` - A reference to the Config struct containing chat IDs
    /// 
    /// # Returns
    /// * `Ok(())` - If message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not logged in, network issues, etc.)
    pub async fn send(
        bot: &SteamBot,
        message: &str,
        config: &Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        Self::send_to_chat(bot, message, config.chat_group_id, config.chat_id).await
    }

    /// Sends a message to a specific Steam group chat room
    /// 
    /// This function sends a message to the specified Steam group chat using
    /// the provided chat_group_id and chat_id. Use this when replying to a
    /// message in the same chat room.
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `message` - The message to send to the Steam group chat
    /// * `chat_group_id` - The chat group ID to send the message to
    /// * `chat_id` - The chat room ID to send the message to
    /// 
    /// # Returns
    /// * `Ok(())` - If message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not logged in, network issues, etc.)
    pub async fn send_to_chat(
        bot: &SteamBot,
        message: &str,
        chat_group_id: u64,
        chat_id: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let session_guard = bot.session.lock().await;

        if session_guard.is_none() {
            return Err("SteamBot not fully initialized. Please login first.".into());
        }
        
        if let Some(ref session) = *session_guard {
            let params = SendGroupMessageParams::new(
                chat_group_id,
                chat_id,
                message,
            );
            session.chat().send_group_message(params).await.map_err(|e| format!("Failed to send message: {}", e))?;
            
            println!("Message sent to Steam chat (group: {}, chat: {}): {}", chat_group_id, chat_id, message);
            Ok(())
        } else {
            Err("Steam chat client not initialized. Please login first.".into())
        }
    }

    /// Sends a message to a specific Steam group chat room with automatic recovery
    /// 
    /// This function sends a message to the specified Steam group chat using
    /// the provided chat_group_id and chat_id, with automatic reconnection
    /// and retry logic on connection failures. Use this when replying to a
    /// message in the same chat room.
    /// 
    /// If a connection error occurs, this function automatically attempts
    /// to reconnect and retry sending the message once.
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `message` - The message to send to the Steam group chat
    /// * `chat_group_id` - The chat group ID to send the message to
    /// * `chat_id` - The chat room ID to send the message to
    /// * `config` - Reference to the Config (needed for reconnection)
    /// 
    /// # Returns
    /// * `Ok(())` - If message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails after retry
    pub async fn send_to_chat_with_recovery(
        bot: &SteamBot,
        message: &str,
        chat_group_id: u64,
        chat_id: u64,
        config: &Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        match Self::send_to_chat(bot, message, chat_group_id, chat_id).await {
            Ok(()) => Ok(()),
            Err(e) => {
                let error_str = e.to_string().to_lowercase();
                if is_connection_error(&error_str) {
                    Self::handle_connection_error(bot, config, e).await?;
                    // Retry sending the message
                    Self::send_to_chat(bot, message, chat_group_id, chat_id).await
                } else {
                    Err(e)
                }
            }
        }
    }

    /// Sends a message using the global SteamBot instance
    /// 
    /// This static method sends a message using the global SteamBot instance
    /// that was initialized by the `main()` function. This allows other parts
    /// of the application to send messages without needing direct access to
    /// a SteamBot instance.
    /// 
    /// # Arguments
    /// * `message` - The message to send to the Steam group chat
    /// 
    /// # Returns
    /// * `Ok(())` - If message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not initialized, not logged in, etc.)
    /// 
    /// # Example
    /// ```rust
    /// // From anywhere in the application
    /// MessageSender::send_global("!sub").await?;
    /// ```
    /// 
    /// # Thread Safety
    /// This function uses OnceCell for thread-safe access to the global SteamBot instance.
    /// The instance must be initialized by calling `main()` first.
    pub async fn send_global(message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(steam_bot) = registry::bot() {
            let config = registry::config();
            Self::send_with_immediate_recovery(steam_bot, message, config).await
        } else {
            Err("SteamBot not initialized".into())
        }
    }

    /// Sends a message to a specific chat room using the global SteamBot instance
    /// 
    /// This static method sends a message to a specific Steam group chat room using
    /// the global SteamBot instance and the provided chat IDs. This allows replying
    /// to messages in any chat room while using the registered bot instance.
    /// 
    /// # Arguments
    /// * `message` - The message to send to the Steam group chat
    /// * `chat_group_id` - The chat group ID to send the message to
    /// * `chat_id` - The chat room ID to send the message to
    /// 
    /// # Returns
    /// * `Ok(())` - If message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not initialized, not logged in, etc.)
    /// 
    /// # Example
    /// ```rust
    /// // Reply to a message in a specific chat room
    /// MessageSender::send_to_chat_global("Hello!", 12345, 67890).await?;
    /// ```
    /// 
    /// # Thread Safety
    /// This function uses OnceCell for thread-safe access to the global SteamBot instance.
    /// The instance must be initialized by calling `main()` first.
    pub async fn send_to_chat_global(
        message: &str,
        chat_group_id: u64,
        chat_id: u64,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(steam_bot) = registry::bot() {
            let config = registry::config();
            Self::send_to_chat_with_recovery(steam_bot, message, chat_group_id, chat_id, config).await
        } else {
            Err("SteamBot not initialized".into())
        }
    }
}

