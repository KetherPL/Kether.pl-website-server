// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use crate::steam_bot::bot::SteamBot;
use crate::steam_bot::connection::ConnectionManager;
use crate::steam_bot::registry;
use colored::Colorize;
use std::sync::Arc;
use tokio::time::Duration;

/// Coordinates the lifecycle of the SteamBot by handling configuration loading,
/// credential validation, and runtime mode selection.
pub struct SteamBotService {
    bot: Arc<SteamBot>,
    config: Config,
}

impl SteamBotService {
    /// Loads configuration and prepares a SteamBot instance.
    pub async fn new() -> Result<Self, String> {
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
    pub async fn run(self) -> Result<(), String> {
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

    /// Lists available Steam chat rooms and prints them to the console
    /// 
    /// # Arguments
    /// * `bot` - Reference to the SteamBot instance
    async fn list_chat_rooms(bot: &SteamBot) {
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
    }

    async fn run_chat_discovery_mode(bot: Arc<SteamBot>) -> Result<(), String> {
        println!("⚠️ Chat group_id and/or chat_id not configured in config.toml");
        println!("   Listing available Steam chat rooms...\n");

        Self::list_chat_rooms(&bot).await;
        drop(bot);

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
                    if let Err(e) = ConnectionManager::ensure_healthy(&bot, config).await {
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

