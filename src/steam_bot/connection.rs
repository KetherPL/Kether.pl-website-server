// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use crate::steam_bot::bot::SteamBot;
use crate::steam_bot::state::ConnectionState;
use crate::steam_bot::utils::{is_connection_error, calculate_backoff_delay, RECONNECT_RETRY_LIMIT};
use SC_Sub_Poster::SendGroupMessageParams;
use tokio::time::{Duration, Instant};

/// Connection manager for SteamBot
/// 
/// This struct provides methods for managing the Steam connection, including
/// health checks, reconnection logic, and connection state management.
pub struct ConnectionManager;

impl ConnectionManager {
    /// Checks if a health check should be performed based on the last check time
    /// 
    /// Health checks are performed every 5 minutes (300 seconds) to avoid
    /// excessive connection testing.
    /// 
    /// # Arguments
    /// * `last_check` - Optional timestamp of the last health check
    /// 
    /// # Returns
    /// `true` if a health check should be performed, `false` otherwise
    fn should_perform_health_check(last_check: Option<Instant>) -> bool {
        let now = Instant::now();
        last_check
            .map(|last| now.duration_since(last).as_secs() >= 300)
            .unwrap_or(true)
    }


    /// Checks the health of the Steam connection
    /// 
    /// This function performs a health check on the Steam connection to determine
    /// if it's still valid and functional. It uses a lightweight operation that
    /// doesn't send actual messages to avoid spamming users.
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// 
    /// # Returns
    /// * `Ok(bool)` - True if connection is healthy, false otherwise
    /// * `Err(Box<dyn std::error::Error>)` - If health check fails
    pub async fn check_health(bot: &SteamBot) -> Result<bool, Box<dyn std::error::Error>> {
        let session_guard = bot.session.lock().await;
        
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

    /// Updates the connection state
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `state` - The new connection state
    async fn update_connection_state(bot: &SteamBot, state: ConnectionState) {
        let mut state_guard = bot.connection_state.lock().await;
        *state_guard = state;
    }

    /// Increments the reconnect attempts counter
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// 
    /// # Returns
    /// The new number of reconnect attempts
    async fn increment_reconnect_attempts(bot: &SteamBot) -> u32 {
        let mut attempts_guard = bot.reconnect_attempts.lock().await;
        *attempts_guard += 1;
        *attempts_guard
    }

    /// Resets the reconnect attempts counter
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    async fn reset_reconnect_attempts(bot: &SteamBot) {
        let mut attempts_guard = bot.reconnect_attempts.lock().await;
        *attempts_guard = 0;
    }

    /// Attempts to reconnect to Steam
    /// 
    /// This function attempts to reconnect to Steam when the connection is lost.
    /// It includes exponential backoff to prevent overwhelming the Steam servers.
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `config` - A reference to the Config struct containing Steam credentials
    /// 
    /// # Returns
    /// * `Ok(())` - If reconnection is successful
    /// * `Err(Box<dyn std::error::Error + Send + Sync>)` - If reconnection fails
    pub async fn reconnect(bot: &SteamBot, config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Update connection state
        Self::update_connection_state(bot, ConnectionState::Reconnecting).await;
        
        // Get current reconnect attempts
        let attempts = Self::increment_reconnect_attempts(bot).await;
        
        println!("Attempting to reconnect to Steam (attempt {})", attempts);
        
        // Calculate backoff delay (exponential backoff with max of 60 seconds)
        tokio::time::sleep(calculate_backoff_delay(attempts)).await;
        
        // Clear existing connections
        {
            let mut session_guard = bot.session.lock().await;
            *session_guard = None;
        }
        
        // Attempt to login again
        let login_success = bot.login(config).await.is_ok();
        if login_success {
            // Reset reconnect attempts on success
            Self::reset_reconnect_attempts(bot).await;
            Self::update_connection_state(bot, ConnectionState::Connected).await;
            println!("Successfully reconnected to Steam");
            Ok(())
        } else {
            // Update connection state to failed
            Self::update_connection_state(bot, ConnectionState::Failed).await;
            println!("Failed to reconnect to Steam");
            Err("Failed to reconnect to Steam".into())
        }
    }

    /// Performs a health check with automatic reconnection retry
    /// 
    /// This function checks the connection health and attempts to reconnect
    /// if necessary. It includes retry logic for failed reconnection attempts.
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `config` - A reference to the Config struct containing Steam credentials
    /// 
    /// # Returns
    /// * `Ok(())` - If connection is healthy or successfully reconnected
    /// * `Err(Box<dyn std::error::Error + Send + Sync>)` - If connection cannot be established
    async fn perform_health_check_with_recovery(
        bot: &SteamBot,
        config: &Config,
    ) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        let is_healthy = match Self::check_health(bot).await {
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
                match Self::reconnect(bot, config).await {
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
        
        Ok(())
    }

    /// Ensures the Steam connection is healthy before sending messages
    /// 
    /// This function checks the connection health and attempts to reconnect
    /// if necessary before sending a message. It includes retry logic for
    /// failed reconnection attempts.
    /// 
    /// Health checks are performed every 5 minutes to avoid excessive testing.
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    /// * `config` - A reference to the Config struct containing Steam credentials
    /// 
    /// # Returns
    /// * `Ok(())` - If connection is healthy or successfully reconnected
    /// * `Err(Box<dyn std::error::Error + Send + Sync>)` - If connection cannot be established
    pub async fn ensure_healthy(bot: &SteamBot, config: &Config) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
        // Check if we need to perform a health check (every 5 minutes)
        let should_check = {
            let mut last_check_guard = bot.last_health_check.lock().await;
            let now = Instant::now();
            let should = Self::should_perform_health_check(*last_check_guard);
            if should {
                *last_check_guard = Some(now);
            }
            should
        };
        
        if should_check {
            Self::perform_health_check_with_recovery(bot, config).await?;
        }
        
        Ok(())
    }
}

