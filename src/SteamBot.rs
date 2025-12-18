// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use colored::Colorize;
use SC_Sub_Poster::{ChatRoomClient, LogOn, SendGroupMessageParams};
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{Duration, Instant};

const CONNECTION_ERROR_TOKENS: [&str; 6] = ["broken pipe", "connection", "network", "timeout", "closed", "io error"];
const RECONNECT_RETRY_LIMIT: u32 = 3;
const MAX_BACKOFF_SECONDS: u64 = 60;

fn is_connection_error(message: &str) -> bool {
    CONNECTION_ERROR_TOKENS.iter().any(|token| message.contains(token))
}

fn calculate_backoff_delay(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1);
    let seconds = std::cmp::min(1u64 << exponent, MAX_BACKOFF_SECONDS);
    Duration::from_secs(seconds)
}

mod registry {
    use super::*;
    use once_cell::sync::OnceCell;
    use std::sync::Arc;

    static STEAM_BOT: OnceCell<Arc<SteamBot>> = OnceCell::new();
    static CONFIG: OnceCell<Config> = OnceCell::new();

    pub fn set_bot(bot: Arc<SteamBot>) {
        STEAM_BOT.set(bot).expect("SteamBot already initialized");
    }

    pub fn bot() -> Option<&'static Arc<SteamBot>> {
        STEAM_BOT.get()
    }

    pub fn set_config(config: Config) {
        CONFIG.set(config).expect("Config already initialized");
    }

    pub fn set_config_if_absent(config: Config) {
        let _ = CONFIG.set(config);
    }

    pub fn config() -> &'static Config {
        CONFIG.get().expect("Config not initialized")
    }
}

/// Represents an authenticated Steam session with both the low-level connection
/// and the chat client. Keeping the two coupled avoids double-locking patterns
/// in the SteamBot and ensures the chat client cannot outlive the connection.
struct SteamSession {
    _logon: LogOn,
    chat: ChatRoomClient,
}

impl SteamSession {
    /// Creates a new session from an authenticated `LogOn`.
    fn new(logon: LogOn) -> Self {
        let chat = ChatRoomClient::new(logon.connection().clone());
        Self { _logon: logon, chat }
    }

    /// Provides access to the chat client.
    fn chat(&self) -> &ChatRoomClient {
        &self.chat
    }

}

impl fmt::Debug for SteamSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SteamSession")
            .field("logon", &"<LogOn>")
            .field("chat", &"<ChatRoomClient>")
            .finish()
    }
}

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
///     let config = Config::load().await?;
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
    session: Arc<Mutex<Option<SteamSession>>>,
    connection_state: Arc<Mutex<ConnectionState>>,
    last_health_check: Arc<Mutex<Option<Instant>>>,
    reconnect_attempts: Arc<Mutex<u32>>,
}

impl std::fmt::Debug for SteamBot {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SteamBot")
            .field("session", &"<SteamSession>")
            .finish()
    }
}

impl SteamBot {
    /// Creates a new SteamBot instance
    /// 
    /// Returns a new SteamBot with an uninitialized Steam session.
    /// The session will be established when `login()` is called.
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
            session: Arc::new(Mutex::new(None)),
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
    /// let config = Config::load().await?;
    /// let steam_bot = SteamBot::new();
    /// steam_bot.login(&config).await?;
    /// ```
    pub async fn login(&self, config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        // Validate credentials before attempting login
        if config.steam_account.is_empty() || config.steam_password.is_empty() {
            return Err("Steam credentials are not configured in config.toml".into());
        }
        
        // Update connection state
        {
            let mut state_guard = self.connection_state.lock().await;
            *state_guard = ConnectionState::Connecting;
        }
        
        // Create and login the Steam client
        let steam_client = LogOn::new(&config.steam_account, &config.steam_password).await?;
        let session = SteamSession::new(steam_client);

        {
            let mut session_guard = self.session.lock().await;
            *session_guard = Some(session);
        }

        // Update connection state to connected
        {
            let mut state_guard = self.connection_state.lock().await;
            *state_guard = ConnectionState::Connected;
        }

        // Log successful login
        println!("SteamBot logged in successfully");

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
    /// let config = Config::load().await?;
    /// let steam_bot = SteamBot::new();
    /// steam_bot.login(&config).await?;
    /// steam_bot.send_message("!sub", &config).await?;
    /// ```
    pub async fn send_message(&self, message: &str, config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let session_guard = self.session.lock().await;

        if session_guard.is_none() {
            return Err("SteamBot not fully initialized. Please login first.".into());
        }
        
        if let Some(ref session) = *session_guard {
            let params = SendGroupMessageParams::new(
                config.chat_group_id,
                config.chat_id,
                message,
            );
            session.chat().send_group_message(params).await.map_err(|e| format!("Failed to send message: {}", e))?;
            
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
        if let Some(steam_bot) = registry::bot() {
            let config = registry::config();
            
            // Try to send message with immediate recovery on connection failures
            match steam_bot.send_message(message, config).await {
                Ok(()) => Ok(()),
                Err(e) => {
                    let error_str = e.to_string().to_lowercase();
                    if is_connection_error(&error_str) {
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
    pub async fn check_connection_health(&self, _config: &Config) -> Result<bool, Box<dyn std::error::Error>> {
        let session_guard = self.session.lock().await;
        
        // Check if a session is initialized
        if session_guard.is_none() {
            return Ok(false);
        }
        
        if let Some(ref session) = *session_guard {
            // Try to send a message to an invalid group ID (this won't actually send anything)
            // but will fail quickly if the connection is dead
            let params = SendGroupMessageParams::new(
                0, // Invalid group ID - won't actually send
                0, // Invalid chat ID - won't actually send  
                "health_check", // Test message that won't be sent
            );
            match tokio::time::timeout(
                Duration::from_secs(5), // 5 second timeout
                session.chat().send_group_message(params)
            ).await {
                Ok(Ok(_)) => {
                    // This shouldn't happen with invalid IDs, but if it does, connection is alive
                    Ok(true)
                }
                Ok(Err(e)) => {
                    // Check if the error indicates connection issues
                    let error_str = e.to_string().to_lowercase();
                    if is_connection_error(&error_str) {
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
        tokio::time::sleep(calculate_backoff_delay(attempts)).await;
        
        // Clear existing connections
        {
            let mut session_guard = self.session.lock().await;
            *session_guard = None;
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
                for attempt in 1..=RECONNECT_RETRY_LIMIT {
                    match self.reconnect(config).await {
                        Ok(()) => {
                            println!("Successfully reconnected to Steam");
                            return Ok(());
                        }
                        Err(e) => {
                            if attempt == RECONNECT_RETRY_LIMIT {
                                return Err(Box::new(std::io::Error::new(
                                    std::io::ErrorKind::Other,
                                    format!("Failed to reconnect after {} attempts: {}", RECONNECT_RETRY_LIMIT, e)
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

/// Coordinates the lifecycle of the SteamBot by handling configuration loading,
/// credential validation, and runtime mode selection.
struct SteamBotService {
    bot: Arc<SteamBot>,
    config: Config,
}

impl SteamBotService {
    /// Loads configuration and prepares a SteamBot instance.
    async fn new() -> Result<Self, String> {
        let config = crate::config::Config::load()
            .await
            .map_err(|e| format!("Failed to load config: {}", e))?;

        Ok(Self {
            bot: Arc::new(SteamBot::new()),
            config,
        })
    }

    /// Entry point for running the service. Chooses between disabled and enabled
    /// modes depending on whether credentials are present.
    async fn run(self) -> Result<(), String> {
        if self.config.steam_account.is_empty() || self.config.steam_password.is_empty() {
            self.run_disabled_mode().await
        } else {
            self.run_enabled_mode().await
        }
    }

    async fn run_disabled_mode(self) -> Result<(), String> {
        let Self { config, .. } = self;

        println!("⚠️ No Steam account username and/or password is provided in the config. Call For Sub won't be available.");
        println!("   To enable Call For Sub functionality, set `steam.bot.username` and `steam.bot.password` in `config.toml`");

        registry::set_config_if_absent(config);

        println!("SteamBot running in disabled mode...");
        Self::wait_for_shutdown_loop(false).await
    }

    async fn run_enabled_mode(self) -> Result<(), String> {
        let Self { bot, config } = self;

        registry::set_bot(bot.clone());
        registry::set_config(config);

        println!("Logging in to Steam...");
        let config_ref = registry::config();
        if let Err(e) = bot.login(config_ref).await {
            return Err(format!("Failed to login: {}", e));
        }

        Self::handle_post_login(bot).await
    }

    async fn handle_post_login(bot: Arc<SteamBot>) -> Result<(), String> {
        println!("Checking for available Steam chat rooms...");

        let config = registry::config();
        if config.chat_group_id == 0 || config.chat_id == 0 {
            Self::run_chat_discovery_mode(bot).await
        } else {
            Self::run_active_mode(bot).await
        }
    }

    async fn run_chat_discovery_mode(bot: Arc<SteamBot>) -> Result<(), String> {
        println!("⚠️ Chat group_id and/or chat_id not configured in config.toml");
        println!("   Listing available Steam chat rooms...\n");

        let session_guard = bot.session.lock().await;
        if let Some(ref session) = *session_guard {
            match session.chat().get_my_chat_rooms().await {
                Ok(chat_rooms) => {
                    println!("{} Found {} chat room(s):", "✓".green(), chat_rooms.len());
                    for (i, room) in chat_rooms.iter().enumerate() {
                        println!("  {}. {} (Group: {})", i + 1, room.chat_name.bold(), room.chat_group_name.bold());
                        println!("     Group ID: {}, Chat ID: {}", room.chat_group_id.to_string().bold(), room.chat_id.to_string().bold());
                    }
                    println!("\nTo enable Call For Sub, update config.toml with:");
                    println!("  [steam.chat]");
                    println!("  group_id = <Group ID from above>");
                    println!("  chat_id = <Chat ID from above>");
                }
                Err(e) => {
                    println!("{} Failed to get chat rooms: {:?}", "✗".red(), e);
                }
            }
        }
        drop(session_guard);

        println!("\nSteamBot running in chat-discovery mode (Call For Sub disabled)...");
        Self::wait_for_shutdown_loop(true).await
    }

    async fn run_active_mode(bot: Arc<SteamBot>) -> Result<(), String> {
        println!("Keeping instance alive...");

        let mut health_check_interval = tokio::time::interval(Duration::from_secs(900)); // Every 15 minutes

        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("Shutdown signal received, stopping SteamBot...");
                    break;
                }
                _ = health_check_interval.tick() => {
                    let config = registry::config();
                    println!("Performing periodic Steam connection health check...");
                    if let Err(e) = bot.ensure_connection(config).await {
                        eprintln!("Periodic health check failed: {}", e);
                    } else {
                        let state = bot.get_connection_state().await;
                        let attempts = bot.get_reconnect_attempts().await;
                        println!("SteamBot health check completed: state={:?}, reconnect_attempts={}", state, attempts);
                    }
                }
            }
        }

        println!("SteamBot shutdown complete");
        Ok(())
    }

    async fn wait_for_shutdown_loop(print_completion: bool) -> Result<(), String> {
        loop {
            tokio::select! {
                _ = tokio::signal::ctrl_c() => {
                    println!("Shutdown signal received, stopping SteamBot...");
                    break;
                }
            }
        }

        if print_completion {
            println!("SteamBot shutdown complete");
        }
        Ok(())
    }
}

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
pub async fn main() -> Result<(), String> {
    SteamBotService::new().await?.run().await
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
        let result = steam_bot.send_message("!sub", &config).await;
        assert!(result.is_ok());
        Ok(())
    }
}