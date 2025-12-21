// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use crate::steam_bot::state::ConnectionState;
use SC_Sub_Poster::{ChatRoomClient, LogOn};
use std::fmt;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::Instant;

/// Represents an authenticated Steam session with both the low-level connection
/// and the chat client. Keeping the two coupled avoids double-locking patterns
/// in the SteamBot and ensures the chat client cannot outlive the connection.
pub struct SteamSession {
    _logon: LogOn,
    chat: ChatRoomClient,
}

impl SteamSession {
    /// Creates a new session from an authenticated `LogOn`.
    pub fn new(logon: LogOn) -> Self {
        let chat = ChatRoomClient::new(logon.connection().clone());
        Self { _logon: logon, chat }
    }

    /// Provides access to the chat client.
    pub fn chat(&self) -> &ChatRoomClient {
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

/// SteamBot - A thread-safe wrapper for Steam chat functionality
/// 
/// This struct provides a safe interface for logging into Steam and sending messages
/// to Steam group chats. It uses Arc<Mutex<>> for thread-safe concurrent access
/// to the Steam client and chat client instances.
/// 
/// # Example
/// ```rust
/// use crate::steam_bot::SteamBot;
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
///     Ok(())
/// }
/// ```
// Store bot Steam ID as a u64 to avoid direct dependency on steamid_ng2
// We can convert it back when needed using SteamID::from(u64)
pub struct SteamBot {
    pub(crate) session: Arc<Mutex<Option<SteamSession>>>,
    pub(crate) connection_state: Arc<Mutex<ConnectionState>>,
    pub(crate) last_health_check: Arc<Mutex<Option<Instant>>>,
    pub(crate) reconnect_attempts: Arc<Mutex<u32>>,
    pub(crate) bot_steam_id: Arc<Mutex<Option<u64>>>,
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
            bot_steam_id: Arc::new(Mutex::new(None)),
        }
    }

    /// Validates Steam credentials before attempting login
    /// 
    /// # Arguments
    /// * `config` - A reference to the Config struct containing Steam credentials
    /// 
    /// # Returns
    /// * `Ok(())` - If credentials are valid
    /// * `Err(Box<dyn std::error::Error>)` - If credentials are missing or invalid
    fn validate_credentials(config: &Config) -> Result<(), Box<dyn std::error::Error>> {
        if config.steam_account.is_empty() || config.steam_password.is_empty() {
            return Err("Steam credentials are not configured in config.toml".into());
        }
        Ok(())
    }

    /// Creates a new Steam session from credentials
    /// 
    /// # Arguments
    /// * `config` - A reference to the Config struct containing Steam credentials
    /// 
    /// # Returns
    /// * `Ok((SteamSession, u64))` - If session creation is successful, returns session and bot Steam ID as u64
    /// * `Err(Box<dyn std::error::Error>)` - If session creation fails
    async fn create_steam_session(config: &Config) -> Result<(SteamSession, u64), Box<dyn std::error::Error>> {
        let steam_client = LogOn::new(&config.steam_account, &config.steam_password).await?;
        let bot_steam_id = steam_client.steam_id();
        // Convert SteamID to u64 for storage
        let bot_steam_id_u64: u64 = bot_steam_id.into();
        Ok((SteamSession::new(steam_client), bot_steam_id_u64))
    }

    /// Updates connection state after successful login
    /// 
    /// # Arguments
    /// * `state` - The new connection state (should be `ConnectionState::Connected`)
    async fn update_state_after_login(&self, state: ConnectionState) {
        let mut state_guard = self.connection_state.lock().await;
        *state_guard = state;
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
        Self::validate_credentials(config)?;
        
        // Update connection state
        {
            let mut state_guard = self.connection_state.lock().await;
            *state_guard = ConnectionState::Connecting;
        }
        
        // Create and login the Steam client
        let (session, bot_steam_id) = Self::create_steam_session(config).await?;

        {
            let mut session_guard = self.session.lock().await;
            *session_guard = Some(session);
        }

        // Store the bot's Steam ID
        {
            let mut steam_id_guard = self.bot_steam_id.lock().await;
            *steam_id_guard = Some(bot_steam_id);
        }

        // Update connection state to connected
        self.update_state_after_login(ConnectionState::Connected).await;

        // Log successful login
        println!("SteamBot logged in successfully");

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

    /// Gets the bot's Steam ID
    /// 
    /// This function returns the Steam ID of the logged-in bot account as a u64.
    /// 
    /// # Returns
    /// * `Some(u64)` - The bot's Steam ID if logged in
    /// * `None` - If not logged in yet
    /// 
    /// # Example
    /// ```rust
    /// if let Some(steam_id) = steam_bot.get_bot_steam_id().await {
    ///     println!("Bot Steam ID: {}", steam_id);
    /// }
    /// ```
    pub async fn get_bot_steam_id(&self) -> Option<u64> {
        let steam_id_guard = self.bot_steam_id.lock().await;
        *steam_id_guard
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

