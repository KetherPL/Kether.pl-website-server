// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use SC_Sub_Poster::{LogOn, ChatRoomClient};
use std::sync::Arc;
use tokio::sync::Mutex;
use once_cell::sync::OnceCell;
use tokio::time::{Duration, Instant};

// Global SteamBot instance - thread-safe initialization
static STEAM_BOT: OnceCell<Arc<SteamBot>> = OnceCell::new();
static CONFIG_CACHE: OnceCell<Config> = OnceCell::new();

/// Connection state for tracking Steam connection health
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    Disconnected,
    Connecting,
    Connected,
    Reconnecting,
    Failed,
}

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
    connection_state: Arc<Mutex<ConnectionState>>,
    last_health_check: Arc<Mutex<Option<Instant>>>,
    reconnect_attempts: Arc<Mutex<u32>>,
}

impl std::fmt::Debug for SteamBot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteamBot")
            .field("steam_client", &"<LogOn>")
            .field("chat_client", &"<ChatRoomClient>")
            .finish()
    }
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
            connection_state: Arc::new(Mutex::new(ConnectionState::Disconnected)),
            last_health_check: Arc::new(Mutex::new(None)),
            reconnect_attempts: Arc::new(Mutex::new(0)),
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
        // Update connection state
        {
            let mut state_guard = self.connection_state.lock().await;
            *state_guard = ConnectionState::Connecting;
        }
        
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
            
            // Update connection state to connected
            {
                let mut state_guard = self.connection_state.lock().await;
                *state_guard = ConnectionState::Connected;
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
    pub async fn send_message(&self, message: &str, config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let steam_client_guard = self.steam_client.lock().await;
        let chat_client_guard = self.chat_client.lock().await;
        
        if steam_client_guard.is_none() || chat_client_guard.is_none() {
            return Err("SteamBot not fully initialized. Please login first.".into());
        }
        
        if let Some(ref chat_client) = *chat_client_guard {
            chat_client.send_group_message(
                config.chat_group_id,
                config.chat_id,
                message,
                false,
            ).await.map_err(|e| format!("Failed to send message: {}", e))?;
            
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
    /// # Thread Safety
    /// This function uses OnceCell for thread-safe access to the global SteamBot instance.
    /// The instance must be initialized by calling `main()` first.
    pub async fn send_message_global(message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(steam_bot) = STEAM_BOT.get() {
            let config = CONFIG_CACHE.get_or_init(|| Config::load().expect("Failed to load config"));
            
            // Try to send message with immediate recovery on connection failures
            match steam_bot.send_message(message, config).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    let error_str = e.to_string().to_lowercase();
                    if error_str.contains("broken pipe") || error_str.contains("connection") || 
                       error_str.contains("network") || error_str.contains("timeout") || 
                       error_str.contains("closed") || error_str.contains("io error") {
                        println!("Message send failed due to connection issue: {}", e);
                        println!("Attempting immediate reconnection and retry...");
                        
                        // Attempt to reconnect and retry once
                        if let Err(reconnect_err) = steam_bot.reconnect(config).await {
                            return Err(format!("Failed to reconnect after message send failure: {}", reconnect_err).into());
                        }
                        
                        // Retry sending the message
                        steam_bot.send_message(message, config).await
                    } else {
                        Err(e)
                    }
                }
            }
        } else {
            Err("SteamBot not initialized".into())
        }
    }

    /// Sends a message using the global SteamBot instance with recovery
    /// 
    /// This static method sends a message using the global SteamBot instance
    /// with automatic connection recovery. If the connection is lost, it will
    /// attempt to reconnect before sending the message.
    /// 
    /// # Arguments
    /// * `message` - The message to send to the Steam group chat
    /// 
    /// # Returns
    /// * `Ok(())` - If message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails after recovery attempts
    /// 
    /// # Example
    /// ```rust
    /// // From anywhere in the application with recovery
    /// SteamBot::send_message_global_with_recovery("!sub").await?;
    /// ```
    /// 
    /// # Thread Safety
    /// This function uses OnceCell for thread-safe access to the global SteamBot instance.
    /// The instance must be initialized by calling `main()` first.
    pub async fn send_message_global_with_recovery(message: &str) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        if let Some(steam_bot) = STEAM_BOT.get() {
            let config = CONFIG_CACHE.get_or_init(|| Config::load().expect("Failed to load config"));
            steam_bot.send_message_with_recovery(message, config).await
        } else {
            Err("SteamBot not initialized".into())
        }
    }

    /// Checks the health of the Steam connection
    /// 
    /// This function performs a health check on the Steam connection to determine
    /// if it's still valid and functional. It uses a lightweight operation that
    /// doesn't send actual messages to avoid spamming users.
    /// 
    /// # Arguments
    /// * `config` - A reference to the Config struct containing chat IDs
    /// 
    /// # Returns
    /// * `Ok(bool)` - True if connection is healthy, false otherwise
    /// * `Err(Box<dyn std::error::Error>)` - If health check fails
    /// 
    /// # Example
    /// ```rust
    /// let is_healthy = steam_bot.check_connection_health(&config).await?;
    /// if !is_healthy {
    ///     steam_bot.reconnect(&config).await?;
    /// }
    /// ```
    pub async fn check_connection_health(&self, config: &Config) -> Result<bool, Box<dyn std::error::Error>> {
        let steam_client_guard = self.steam_client.lock().await;
        let chat_client_guard = self.chat_client.lock().await;
        
        // Check if both clients are initialized
        if steam_client_guard.is_none() || chat_client_guard.is_none() {
            return Ok(false);
        }
        
        // Perform a lightweight health check without sending messages
        // We'll use a timeout-based approach to detect dead connections
        if let Some(ref chat_client) = *chat_client_guard {
            // Try to send a message to an invalid group ID (this won't actually send anything)
            // but will fail quickly if the connection is dead
            match tokio::time::timeout(
                Duration::from_secs(5), // 5 second timeout
                chat_client.send_group_message(
                    0, // Invalid group ID - won't actually send
                    0, // Invalid chat ID - won't actually send  
                    "health_check", // Test message that won't be sent
                    false,
                )
            ).await {
                Ok(Ok(_)) => {
                    // This shouldn't happen with invalid IDs, but if it does, connection is alive
                    Ok(true)
                }
                Ok(Err(e)) => {
                    // Check if the error indicates connection issues
                    let error_str = e.to_string().to_lowercase();
                    if error_str.contains("broken pipe") || error_str.contains("connection") || 
                       error_str.contains("network") || error_str.contains("timeout") || 
                       error_str.contains("closed") || error_str.contains("io error") {
                        println!("Connection health check failed: {}", e);
                        Ok(false)
                    } else {
                        // Error is likely due to invalid group/chat IDs, which means connection is alive
                        Ok(true)
                    }
                }
                Err(_timeout) => {
                    // Timeout indicates connection is likely dead
                    println!("Connection health check timed out - connection appears dead");
                    Ok(false)
                }
            }
        } else {
            Ok(false)
        }
    }

    /// Attempts to reconnect to Steam
    /// 
    /// This function attempts to reconnect to Steam when the connection is lost.
    /// It includes exponential backoff to prevent overwhelming the Steam servers.
    /// 
    /// # Arguments
    /// * `config` - A reference to the Config struct containing Steam credentials
    /// 
    /// # Returns
    /// * `Ok(())` - If reconnection is successful
    /// * `Err(Box<dyn std::error::Error>)` - If reconnection fails
    /// 
    /// # Example
    /// ```rust
    /// if !steam_bot.check_connection_health().await? {
    ///     steam_bot.reconnect(&config).await?;
    /// }
    /// ```
    pub async fn reconnect(&self, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        // Update connection state
        {
            let mut state_guard = self.connection_state.lock().await;
            *state_guard = ConnectionState::Reconnecting;
        }
        
        // Get current reconnect attempts
        let attempts = {
            let mut attempts_guard = self.reconnect_attempts.lock().await;
            *attempts_guard += 1;
            *attempts_guard
        };
        
        println!("Attempting to reconnect to Steam (attempt {})", attempts);
        
        // Calculate backoff delay (exponential backoff with max of 60 seconds)
        let backoff_delay = std::cmp::min(2u64.pow(attempts.saturating_sub(1)), 60);
        tokio::time::sleep(Duration::from_secs(backoff_delay)).await;
        
        // Clear existing connections
        {
            let mut steam_guard = self.steam_client.lock().await;
            let mut chat_guard = self.chat_client.lock().await;
            *steam_guard = None;
            *chat_guard = None;
        }
        
        // Attempt to login again
        let login_success = self.login(config).await.is_ok();
        if login_success {
            // Reset reconnect attempts on success
            {
                let mut attempts_guard = self.reconnect_attempts.lock().await;
                *attempts_guard = 0;
            }
            {
                let mut state_guard = self.connection_state.lock().await;
                *state_guard = ConnectionState::Connected;
            }
            println!("Successfully reconnected to Steam");
            Ok(())
        } else {
            // Update connection state to failed
            {
                let mut state_guard = self.connection_state.lock().await;
                *state_guard = ConnectionState::Failed;
            }
            println!("Failed to reconnect to Steam");
            Err("Failed to reconnect to Steam".into())
        }
    }

    /// Ensures the Steam connection is healthy before sending messages
    /// 
    /// This function checks the connection health and attempts to reconnect
    /// if necessary before sending a message. It includes retry logic for
    /// failed reconnection attempts.
    /// 
    /// # Arguments
    /// * `config` - A reference to the Config struct containing Steam credentials
    /// 
    /// # Returns
    /// * `Ok(())` - If connection is healthy or successfully reconnected
    /// * `Err(Box<dyn std::error::Error>)` - If connection cannot be established
    /// 
    /// # Example
    /// ```rust
    /// steam_bot.ensure_connection(&config).await?;
    /// steam_bot.send_message("!sub", &config).await?;
    /// ```
    pub async fn ensure_connection(&self, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        // Check if we need to perform a health check (every 5 minutes)
        let should_check = {
            let mut last_check_guard = self.last_health_check.lock().await;
            let now = Instant::now();
            let should = last_check_guard
                .map(|last| now.duration_since(last).as_secs() >= 300) // Check every 5 minutes
                .unwrap_or(true);
            if should {
                *last_check_guard = Some(now);
            }
            should
        };
        
        if should_check {
            let is_healthy = match self.check_connection_health(config).await {
                Ok(healthy) => healthy,
                Err(e) => {
                    println!("Health check error: {}", e);
                    false // Treat health check errors as unhealthy
                }
            };
            
            if !is_healthy {
                println!("Steam connection unhealthy, attempting reconnection...");
                // Attempt reconnection with retry logic
                let max_retries = 3;
                for attempt in 1..=max_retries {
                    match self.reconnect(config).await {
                        Ok(()) => {
                            println!("Successfully reconnected to Steam");
                            return Ok(());
                        }
                        Err(e) => {
                            if attempt == max_retries {
                                return Err(Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other, 
                                    format!("Failed to reconnect after {} attempts: {}", max_retries, e)
                                )));
                            }
                            println!("Reconnection attempt {} failed, retrying... (Error: {})", attempt, e);
                        }
                    }
                }
            } else {
                println!("Steam connection health check passed");
            }
        }
        
        Ok(())
    }

    /// Sends a message with automatic connection recovery
    /// 
    /// This function sends a message to the Steam group chat with automatic
    /// connection recovery. If the connection is lost, it will attempt to
    /// reconnect before sending the message.
    /// 
    /// # Arguments
    /// * `message` - The message to send to the Steam group chat
    /// * `config` - A reference to the Config struct containing chat IDs
    /// 
    /// # Returns
    /// * `Ok(())` - If message is sent successfully
    /// * `Err(Box<dyn std::error::Error>)` - If sending fails after recovery attempts
    /// 
    /// # Example
    /// ```rust
    /// steam_bot.send_message_with_recovery("!sub", &config).await?;
    /// ```
    pub async fn send_message_with_recovery(&self, message: &str, config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Ensure connection is healthy
        self.ensure_connection(config).await.map_err(|e| format!("Connection check failed: {}", e))?;
        
        // Send message with retry logic
        let max_retries = 3;
        for attempt in 1..=max_retries {
            match self.send_message(message, config).await {
                Ok(()) => return Ok(()),
                Err(e) => {
                    if attempt == max_retries {
                        return Err(format!("Failed to send message after {} attempts: {}", max_retries, e).into());
                    }
                    println!("Message send attempt {} failed, retrying...", attempt);
                    
                    // Try to reconnect before next attempt
                    if let Err(reconnect_err) = self.reconnect(config).await {
                        println!("Failed to reconnect during retry: {}", reconnect_err);
                    }
                }
            }
        }
        
        Err("Failed to send message after all retry attempts".into())
    }

    /// Gets the current connection state
    /// 
    /// This function returns the current state of the Steam connection.
    /// 
    /// # Returns
    /// The current ConnectionState
    /// 
    /// # Example
    /// ```rust
    /// let state = steam_bot.get_connection_state().await;
    /// println!("Current connection state: {:?}", state);
    /// ```
    pub async fn get_connection_state(&self) -> ConnectionState {
        let state_guard = self.connection_state.lock().await;
        state_guard.clone()
    }

    /// Gets the number of reconnect attempts
    /// 
    /// This function returns the number of reconnection attempts made.
    /// 
    /// # Returns
    /// The number of reconnect attempts
    /// 
    /// # Example
    /// ```rust
    /// let attempts = steam_bot.get_reconnect_attempts().await;
    /// println!("Reconnect attempts: {}", attempts);
    /// ```
    pub async fn get_reconnect_attempts(&self) -> u32 {
        let attempts_guard = self.reconnect_attempts.lock().await;
        *attempts_guard
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
/// 5. Keeps the instance alive with periodic health checks
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
pub async fn main() -> Result<(), String> {
    // Load configuration
    let config = match crate::config::Config::load() {
        Ok(config) => config,
        Err(e) => return Err(format!("Failed to load config: {}", e)),
    };
    
    // Create SteamBot instance
    let steam_bot = Arc::new(SteamBot::new());
    
    // Initialize global instance
    STEAM_BOT.set(steam_bot.clone()).unwrap();
    CONFIG_CACHE.set(config).unwrap();
    
    // Login to Steam
    println!("Logging in to Steam...");
    if let Err(e) = steam_bot.login(&CONFIG_CACHE.get().unwrap()).await {
        return Err(format!("Failed to login: {}", e));
    }
    
    println!("SteamBot logged in successfully. Keeping instance alive...");
    
    // Keep the SteamBot instance alive with periodic health checks
    let mut health_check_interval = tokio::time::interval(Duration::from_secs(900)); // Every 15 minutes
    
    loop {
        tokio::select! {
            // Handle shutdown signal
            _ = tokio::signal::ctrl_c() => {
                println!("Shutdown signal received, stopping SteamBot...");
                break;
            }
            // Periodic health check
            _ = health_check_interval.tick() => {
                let config = CONFIG_CACHE.get().unwrap();
                println!("Performing periodic Steam connection health check...");
                if let Err(e) = steam_bot.ensure_connection(config).await {
                    eprintln!("Periodic health check failed: {}", e);
                } else {
                    let state = steam_bot.get_connection_state().await;
                    let attempts = steam_bot.get_reconnect_attempts().await;
                    println!("SteamBot health check completed: state={:?}, reconnect_attempts={}", state, attempts);
                }
            }
        }
    }
    
    println!("SteamBot shutdown complete");
    Ok(())
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