// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use crate::steam_bot::bot::SteamBot;
use crate::steam_bot::connection::ConnectionManager;
use crate::steam_bot::registry;
use crate::steam_bot::utils::is_connection_error;
use SC_Sub_Poster::{SendGroupMessageParams, PreprocessedMessage};

/// Small delay before delete so the echoed message is committed server-side.
const DELETE_SETTLE_DELAY_MS: u64 = 500;

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
    /// * `Ok(u64)` - The message ordinal if message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails after retry
    async fn send_with_immediate_recovery(
        bot: &SteamBot,
        message: &str,
        config: &Config,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        match Self::send(bot, message, config).await {
            Ok(ordinal) => Ok(ordinal),
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
    /// * `Ok(u64)` - The message ordinal if message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not logged in, network issues, etc.)
    pub async fn send(
        bot: &SteamBot,
        message: &str,
        config: &Config,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
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
    /// * `Ok(u64)` - The message ordinal if message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not logged in, network issues, etc.)
    pub async fn send_to_chat(
        bot: &SteamBot,
        message: &str,
        chat_group_id: u64,
        chat_id: u64,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let preprocessed = Self::send_to_chat_with_preprocessed(bot, message, chat_group_id, chat_id).await?;
        Ok(preprocessed.ordinal.map(|o| o as u64).unwrap_or(0))
    }

    /// Sends a message to a specific Steam group chat room and returns the PreprocessedMessage
    /// 
    /// This function sends a message to the specified Steam group chat using
    /// the provided chat_group_id and chat_id. Returns the PreprocessedMessage
    /// which contains both ordinal and server_timestamp needed for deletion.
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `message` - The message to send to the Steam group chat
    /// * `chat_group_id` - The chat group ID to send the message to
    /// * `chat_id` - The chat room ID to send the message to
    /// 
    /// # Returns
    /// * `Ok(PreprocessedMessage)` - The preprocessed message with ordinal and server_timestamp
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not logged in, network issues, etc.)
    pub async fn send_to_chat_with_preprocessed(
        bot: &SteamBot,
        message: &str,
        chat_group_id: u64,
        chat_id: u64,
    ) -> Result<PreprocessedMessage, Box<dyn std::error::Error + Send + Sync>> {
        let session_guard = bot.session.lock().await;
        let Some(ref session) = *session_guard else {
            return Err("SteamBot not fully initialized. Please login first.".into());
        };

        let params = SendGroupMessageParams::new(
            chat_group_id,
            chat_id,
            message,
        )
        .with_echo_to_sender(true);
        let preprocessed = session
            .chat()
            .send_group_message(params)
            .await
            .map_err(|e| format!("Failed to send message: {}", e))?;

        // Extract ordinal and server_timestamp from PreprocessedMessage
        // When echo_to_sender is enabled, send_group_message waits for the echo notification
        // which provides both values. For deletion: server_timestamp is required, ordinal can be 0 (will be omitted).
        let ordinal = preprocessed.ordinal.map(|o| o as u64).unwrap_or(0);
        let server_timestamp = preprocessed.server_timestamp.map(|t| t as u64).unwrap_or(0);

        // server_timestamp is required for deletion, ordinal can be 0 (it will be omitted per Steam API)
        if server_timestamp == 0 {
            eprintln!("Warning: Message sent but server_timestamp not available (ordinal: {}, server_timestamp: {}). Message deletion will be skipped. Ensure echo_to_sender is enabled and echo notification was received.", ordinal, server_timestamp);
        // } else {
        //     println!("Message sent to Steam chat (group: {}, chat: {}): {} (ordinal: {}, server_timestamp: {})", chat_group_id, chat_id, message, ordinal, server_timestamp);
        }

        Ok(preprocessed)
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
    /// * `Ok(u64)` - The message ordinal if message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not initialized, not logged in, etc.)
    /// 
    /// # Example
    /// ```rust
    /// // From anywhere in the application
    /// let ordinal = MessageSender::send_global("!sub").await?;
    /// ```
    /// 
    /// # Thread Safety
    /// This function uses OnceCell for thread-safe access to the global SteamBot instance.
    /// The instance must be initialized by calling `main()` first.
    pub async fn send_global(message: &str) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let Some(steam_bot) = registry::bot() else {
            return Err("SteamBot not initialized".into());
        };
        let config = registry::config();
        Self::send_with_immediate_recovery(&steam_bot, message, &config).await
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
    /// * `Ok(u64)` - The message ordinal if message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not initialized, not logged in, etc.)
    /// 
    /// # Example
    /// ```rust
    /// // Reply to a message in a specific chat room
    /// let ordinal = MessageSender::send_to_chat_global("Hello!", 12345, 67890).await?;
    /// ```
    /// 
    /// # Thread Safety
    /// This function uses OnceCell for thread-safe access to the global SteamBot instance.
    /// The instance must be initialized by calling `main()` first.
    pub async fn send_to_chat_global(
        message: &str,
        chat_group_id: u64,
        chat_id: u64,
    ) -> Result<u64, Box<dyn std::error::Error + Send + Sync>> {
        let preprocessed = Self::send_to_chat_global_with_preprocessed(message, chat_group_id, chat_id).await?;
        Ok(preprocessed.ordinal.map(|o| o as u64).unwrap_or(0))
    }

    /// Sends a message to a specific chat room using the global SteamBot instance and returns PreprocessedMessage
    /// 
    /// This static method sends a message to a specific Steam group chat room using
    /// the global SteamBot instance and the provided chat IDs. Returns PreprocessedMessage
    /// which contains both ordinal and server_timestamp needed for deletion.
    /// 
    /// # Arguments
    /// * `message` - The message to send to the Steam group chat
    /// * `chat_group_id` - The chat group ID to send the message to
    /// * `chat_id` - The chat room ID to send the message to
    /// 
    /// # Returns
    /// * `Ok(PreprocessedMessage)` - The preprocessed message with ordinal and server_timestamp
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not initialized, not logged in, etc.)
    pub async fn send_to_chat_global_with_preprocessed(
        message: &str,
        chat_group_id: u64,
        chat_id: u64,
    ) -> Result<PreprocessedMessage, Box<dyn std::error::Error + Send + Sync>> {
        let Some(steam_bot) = registry::bot() else {
            return Err("SteamBot not initialized".into());
        };
        let config = registry::config();
        Self::send_to_chat_with_recovery_preprocessed(&steam_bot, message, chat_group_id, chat_id, &config).await
    }

    /// Sends a message to a specific Steam group chat room with automatic recovery (returns PreprocessedMessage)
    async fn send_to_chat_with_recovery_preprocessed(
        bot: &SteamBot,
        message: &str,
        chat_group_id: u64,
        chat_id: u64,
        config: &Config,
    ) -> Result<PreprocessedMessage, Box<dyn std::error::Error + Send + Sync>> {
        match Self::send_to_chat_with_preprocessed(bot, message, chat_group_id, chat_id).await {
            Ok(preprocessed) => Ok(preprocessed),
            Err(e) => {
                let error_str = format!("{}", e);
                if is_connection_error(&error_str) {
                    Self::handle_connection_error(bot, config, e).await?;
                    Self::send_to_chat_with_preprocessed(bot, message, chat_group_id, chat_id).await
                } else {
                    Err(error_str.into())
                }
            }
        }
    }

    /// Deletes a message from a Steam group chat room using PreprocessedMessage
    /// 
    /// This function deletes a message using the PreprocessedMessage returned from
    /// send_group_message. Steam requires both ordinal and server_timestamp to be
    /// non-zero for deletion, which are both contained in PreprocessedMessage.
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `chat_group_id` - The chat group ID where the message is located
    /// * `chat_id` - The chat room ID where the message is located
    /// * `preprocessed` - The PreprocessedMessage returned from send_group_message
    /// 
    /// # Returns
    /// * `Ok(())` - If message is deleted successfully
    /// * `Err(Box<dyn std::error::Error>)` - If deletion fails
    pub async fn delete_message(
        bot: &SteamBot,
        chat_group_id: u64,
        chat_id: u64,
        preprocessed: PreprocessedMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let session_guard = bot.session.lock().await;
        let Some(ref session) = *session_guard else {
            return Err("SteamBot not fully initialized. Please login first.".into());
        };

        // Extract ordinal and server_timestamp from PreprocessedMessage
        // According to Steam API: ordinal can be omitted if 0, but server_timestamp is required
        let ordinal = preprocessed.ordinal.map(|o| o as u64).unwrap_or(0);
        let server_timestamp = preprocessed.server_timestamp.map(|t| t as u64).unwrap_or(0);

        // server_timestamp is required, ordinal can be 0 (it will be omitted in the deletion request)
        if server_timestamp == 0 {
            eprintln!("Cannot delete message: server_timestamp is required but not available (ordinal: {}, server_timestamp: {})", ordinal, server_timestamp);
            return Err(format!("Message deletion requires server_timestamp to be non-zero (ordinal: {}, server_timestamp: {})", ordinal, server_timestamp).into());
        }

        // Small delay to ensure the message is fully processed by Steam before deletion
        tokio::time::sleep(tokio::time::Duration::from_millis(DELETE_SETTLE_DELAY_MS)).await;

        // Use delete_group_messages_from_preprocessed which extracts both ordinal and server_timestamp
        // from the PreprocessedMessage objects
        let messages = vec![preprocessed];

        session
            .chat()
            .delete_group_messages_from_preprocessed(chat_group_id, chat_id, messages)
            .await
            .map_err(|e| {
                let error_str = format!("{}", e);
                eprintln!("Delete message error details - group: {}, chat: {}, ordinal: {}, server_timestamp: {}, error: {}",
                         chat_group_id, chat_id, ordinal, server_timestamp, error_str);
                format!("Failed to delete message (ordinal: {}, server_timestamp: {}): {}", ordinal, server_timestamp, error_str)
            })?;

        // println!("Message deleted from Steam chat (group: {}, chat: {}, ordinal: {}, server_timestamp: {})", chat_group_id, chat_id, ordinal, server_timestamp);
        Ok(())
    }

    /// Deletes a message from a Steam group chat room using the global SteamBot instance
    /// 
    /// This is a convenience function that uses the global SteamBot instance
    /// and the provided chat IDs and PreprocessedMessage.
    /// 
    /// # Arguments
    /// * `chat_group_id` - The chat group ID where the message is located
    /// * `chat_id` - The chat room ID where the message is located
    /// * `preprocessed` - The PreprocessedMessage returned from send_to_chat_with_preprocessed
    /// 
    /// # Returns
    /// * `Ok(())` - If message is deleted successfully
    /// * `Err(Box<dyn std::error::Error>)` - If deletion fails
    pub async fn delete_message_global(
        chat_group_id: u64,
        chat_id: u64,
        preprocessed: PreprocessedMessage,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let Some(steam_bot) = registry::bot() else {
            return Err("SteamBot not initialized".into());
        };
        Self::delete_message(&steam_bot, chat_group_id, chat_id, preprocessed).await
    }

    /// Deletes a single message from a Steam group chat room using explicit identifiers.
    ///
    /// This is intended for incoming-message moderation workflows where message identifiers
    /// come from chat notifications rather than from `PreprocessedMessage`.
    ///
    /// # Arguments
    /// * `chat_group_id` - The chat group ID where the message is located
    /// * `chat_id` - The chat room ID where the message is located
    /// * `server_timestamp` - Steam server timestamp of the message (required, non-zero)
    /// * `ordinal` - Message ordinal; can be zero
    ///
    /// # Returns
    /// * `Ok(())` - If message is deleted successfully
    /// * `Err(Box<dyn std::error::Error>)` - If deletion fails
    pub async fn delete_group_message_by_id_global(
        chat_group_id: u64,
        chat_id: u64,
        server_timestamp: u32,
        ordinal: u32,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if server_timestamp == 0 {
            return Err("Cannot delete message: server_timestamp is zero".into());
        }

        let Some(steam_bot) = registry::bot() else {
            return Err("SteamBot not initialized".into());
        };

        let session_guard = steam_bot.session.lock().await;
        let Some(ref session) = *session_guard else {
            return Err("SteamBot not fully initialized. Please login first.".into());
        };

        session
            .chat()
            .delete_group_messages(chat_group_id, chat_id, vec![(server_timestamp, ordinal)])
            .await
            .map_err(|e| {
                format!(
                    "Failed to delete message (group: {}, chat: {}, ts: {}, ordinal: {}): {}",
                    chat_group_id, chat_id, server_timestamp, ordinal, e
                )
            })?;

        Ok(())
    }
}

