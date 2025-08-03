// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use SC_Sub_Poster::{LogOn, ChatRoomClient};
use std::sync::Arc;
use tokio::sync::Mutex;

// Global SteamBot instance
static mut STEAM_BOT: Option<Arc<SteamBot>> = None;

/// SteamBot - A thread-safe wrapper for Steam chat functionality
/// 
/// This struct provides a safe interface for logging into Steam and sending messages
/// to Steam group chats. It uses Arc<Mutex<>> for thread-safe concurrent access
/// to the Steam client and chat client instances.
/// 
/// # Example
/// ```rust
/// use crate::SteamBot::SteamBot;
/// use crate::config::Config;
/// 
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     let config = Config::load()?;
///     let steam_bot = SteamBot::new();
///     
///     // Login to Steam
///     steam_bot.login(&config).await?;
///     
///     // Send a message
///     steam_bot.send_message("!sub", &config).await?;
///     
///     Ok(())
/// }
/// ```
pub struct SteamBot {
    steam_client: Arc<Mutex<Option<LogOn>>>,
    chat_client: Arc<Mutex<Option<ChatRoomClient>>>,
}

impl SteamBot {
    /// Creates a new SteamBot instance
    /// 
    /// Returns a new SteamBot with uninitialized steam_client and chat_client.
    /// The clients will be initialized when `login()` is called.
    /// 
    /// # Returns
    /// A new `SteamBot` instance ready for login
    /// 
    /// # Example
    /// ```rust
    /// let steam_bot = SteamBot::new();
    /// ```
    pub fn new() -> Self {
        Self {
            steam_client: Arc::new(Mutex::new(None)),
            chat_client: Arc::new(Mutex::new(None)),
        }
    }

    /// Logs into Steam using the provided configuration
    /// 
    /// This function authenticates with Steam using the account credentials
    /// from the config file. Upon successful login, it initializes both the
    /// steam client and chat client for message sending.
    /// 
    /// # Arguments
    /// * `config` - A reference to the Config struct containing Steam credentials
    /// 
    /// # Returns
    /// * `Ok(())` - If login is successful
    /// * `Err(Box<dyn std::error::Error>)` - If login fails (invalid credentials, network issues, etc.)
    /// 
    /// # Example
    /// ```rust
    /// let config = Config::load()?;
    /// let steam_bot = SteamBot::new();
    /// steam_bot.login(&config).await?;
    /// ```
    pub async fn login(&self, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        // Create and login the Steam client
        let steam_client = LogOn::new(&config.steam_account, &config.steam_password).await?;
        
        // Store the steam client in the mutex
        {
            let mut steam_client_guard = self.steam_client.lock().await;
            *steam_client_guard = Some(steam_client);
        }

        // Create chat client from the steam client
        let steam_client_guard = self.steam_client.lock().await;
        if let Some(ref steam_client) = *steam_client_guard {
            let chat_client = ChatRoomClient::new(steam_client.connection().clone());
            
            // Store the chat client in the mutex
            {
                let mut chat_client_guard = self.chat_client.lock().await;
                *chat_client_guard = Some(chat_client);
            }
            
            // Log successful login
            println!("SteamBot logged in successfully");
        }

        Ok(())
    }

    /// Sends a message to the Steam group chat
    /// 
    /// This function sends a message to the specified Steam group chat using
    /// the chat_group_id and chat_id from the config. The message will be
    /// sent to all members of the group chat.
    /// 
    /// # Arguments
    /// * `message` - The message to send to the Steam group chat
    /// * `config` - A reference to the Config struct containing chat IDs
    /// 
    /// # Returns
    /// * `Ok(())` - If message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails (not logged in, network issues, etc.)
    /// 
    /// # Example
    /// ```rust
    /// let config = Config::load()?;
    /// let steam_bot = SteamBot::new();
    /// steam_bot.login(&config).await?;
    /// steam_bot.send_message("!sub", &config).await?;
    /// ```
    pub async fn send_message(&self, message: &str, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        let chat_client_guard = self.chat_client.lock().await;
        
        if let Some(ref chat_client) = *chat_client_guard {
            chat_client.send_group_message(
                config.chat_group_id,
                config.chat_id,
                message,
                false, // echo_to_sender
            ).await?;
            
            println!("Message sent to Steam chat: {}", message);
        } else {
            return Err("Steam chat client not initialized. Please login first.".into());
        }

        Ok(())
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
    /// SteamBot::send_message_global("!sub").await?;
    /// ```
    /// 
    /// # Safety
    /// This function uses unsafe code to access the global SteamBot instance.
    /// The instance must be initialized by calling `main()` first.
    pub async fn send_message_global(message: &str) -> Result<(), Box<dyn std::error::Error>> {
        unsafe {
            if let Some(ref steam_bot) = STEAM_BOT {
                let config = crate::config::Config::load()?;
                steam_bot.send_message(message, &config).await
            } else {
                Err("SteamBot not initialized".into())
            }
        }
    }
}

/// Default implementation for SteamBot
/// 
/// Creates a new SteamBot instance using the `new()` method.
/// This allows SteamBot to be used in contexts that require the Default trait.
impl Default for SteamBot {
    fn default() -> Self {
        Self::new()
    }
}

/// Main entry point for the SteamBot service
/// 
/// This function initializes the SteamBot, logs into Steam, and keeps the
/// instance alive indefinitely. It also sets up the global SteamBot instance
/// that can be accessed by other parts of the application.
/// 
/// The function performs the following steps:
/// 1. Loads the configuration from KISS.ini
/// 2. Creates a new SteamBot instance
/// 3. Initializes the global SteamBot instance for external access
/// 4. Logs into Steam using the provided credentials
/// 5. Keeps the instance alive in an infinite loop
/// 
/// # Returns
/// * `Ok(())` - Never returns successfully (runs indefinitely)
/// * `Err(Box<dyn std::error::Error>)` - If initialization or login fails
/// 
/// # Example
/// ```rust
/// #[tokio::main]
/// async fn main() -> Result<(), Box<dyn std::error::Error>> {
///     SteamBot::main().await
/// }
/// ```
pub async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration
    let config = crate::config::Config::load()?;
    
    // Create SteamBot instance
    let steam_bot = Arc::new(SteamBot::new());
    
    // Initialize global instance
    unsafe {
        STEAM_BOT = Some(steam_bot.clone());
    }
    
    // Login to Steam
    println!("Logging in to Steam...");
    steam_bot.login(&config).await?;
    
    println!("SteamBot logged in successfully. Keeping instance alive...");
    
    // Keep the SteamBot instance alive
    loop {
        tokio::time::sleep(tokio::time::Duration::from_secs(60)).await;
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
    /// This test requires valid Steam credentials in the KISS.ini file.
    /// The test will fail if the credentials are invalid or if the Steam
    /// service is unavailable.
    #[tokio::test]
    async fn test_send_message() -> Result<(), Box<dyn std::error::Error>> {
        let config = Config::load()?;
        let steam_bot = SteamBot::new();
        steam_bot.login(&config).await?;
        let result = steam_bot.send_message("!sub", &config).await;
        assert!(result.is_ok());
        Ok(())
    }
}