// SPDX-License-Identifier: GPL-3.0-only

//! SteamBot module for managing Steam chat functionality
//! 
//! This module provides a modular, thread-safe interface for logging into Steam
//! and sending messages to Steam group chats. It includes connection management,
//! health checks, automatic reconnection, and message sending with retry logic.
//! 
//! # Module Structure
//! 
//! - `bot` - Core SteamBot struct and basic operations (login, session management)
//! - `connection` - Connection health checks, reconnection logic, state management
//! - `messaging` - Message sending with retry logic
//! - `service` - Service lifecycle and mode management
//! - `registry` - Global state management (singleton pattern)
//! - `state` - ConnectionState enum and related types
//! - `utils` - Utility functions (error detection, backoff calculation)
//! 
//! # Example
//! ```rust
//! use crate::steam_bot::SteamBot;
//! use crate::config::Config;
//! 
//! #[tokio::main]
//! async fn main() -> Result<(), Box<dyn std::error::Error>> {
//!     let config = Config::load().await?;
//!     let steam_bot = SteamBot::new();
//!     
//!     // Login to Steam
//!     steam_bot.login(&config).await?;
//!     
//!     Ok(())
//! }
//! ```

mod bot;
mod commands;
mod connection;
mod listener;
mod messaging;
pub mod plan_broadcast;
pub(crate) mod registry;
mod service;
mod state;
mod utils;

// Re-export public types and functions
pub use bot::SteamBot;
pub use messaging::MessageSender;

/// Main entry point for the SteamBot service
/// 
/// This function initializes the SteamBot, logs into Steam, and keeps the
/// instance alive indefinitely. It also sets up the global SteamBot instance
/// that can be accessed by other parts of the application.
/// 
/// The function performs the following steps:
/// 1. Loads the configuration from config.toml
/// 2. Validates Steam credentials are provided
/// 3. Creates a new SteamBot instance
/// 4. Initializes the global SteamBot instance for external access
/// 5. Logs into Steam using the provided credentials
/// 6. Keeps the instance alive with periodic health checks
/// 
/// # Returns
/// * `Ok(())` - Never returns successfully (runs indefinitely)
/// * `Err(String)` - If initialization or login fails
/// 
/// # Example
/// ```rust
/// #[tokio::main]
/// async fn main() -> Result<(), String> {
///     SteamBot::main().await
/// }
/// ```
pub async fn run(shutdown: tokio::sync::broadcast::Receiver<()>) -> Result<(), String> {
    service::SteamBotService::new().await?.run(shutdown).await
}

/// Main entry point for the SteamBot service (runs until Ctrl+C).
pub async fn main() -> Result<(), String> {
    let (_shutdown_tx, shutdown_rx) = tokio::sync::broadcast::channel(1);
    run(shutdown_rx).await
}

// Public API methods
impl SteamBot {
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
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails
    /// 
    /// # Example
    /// ```rust
    /// // From anywhere in the application
    /// SteamBot::send_message_global("!sub").await?;
    /// ```
    pub async fn send_message_global(message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let _ = MessageSender::send_global(message).await?;
        Ok(())
    }
}

/// Test module for SteamBot functionality
/// 
/// This module contains unit tests for the SteamBot implementation.
/// Tests are only compiled when the `test` feature is enabled.
#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    /// Tests the message sending functionality
    /// 
    /// This test verifies that the SteamBot can:
    /// 1. Load configuration successfully
    /// 2. Create a new SteamBot instance
    /// 3. Login to Steam
    /// 4. Send a message to the Steam group chat
    /// 
    /// # Returns
    /// * `Ok(())` - If all operations succeed
    /// * `Err(Box<dyn std::error::Error>)` - If any operation fails
    /// 
    /// # Note
    /// This test requires valid Steam credentials in the config.toml file.
    /// The test will fail if the credentials are invalid or if the Steam
    /// service is unavailable.
    #[tokio::test]
    async fn test_send_message() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::load().await?;
        let steam_bot = SteamBot::new();
        steam_bot.login(&config).await?;
        let result = MessageSender::send(&steam_bot, "!sub", &config).await;
        assert!(result.is_ok());
        Ok(())
    }
}

