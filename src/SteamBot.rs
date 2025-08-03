// SPDX-License-Identifier: GPL-3.0-only

use crate::config::Config;
use SC_Sub_Poster::{LogOn, ChatRoomClient};
use std::sync::Arc;
use tokio::sync::Mutex;

// Global SteamBot instance
static mut STEAM_BOT: Option<Arc<SteamBot>> = None;

pub struct SteamBot {
    steam_client: Arc<Mutex<Option<LogOn>>>,
    chat_client: Arc<Mutex<Option<ChatRoomClient>>>,
}

impl SteamBot {
    pub fn new() -> Self {
        Self {
            steam_client: Arc::new(Mutex::new(None)),
            chat_client: Arc::new(Mutex::new(None)),
        }
    }

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

impl Default for SteamBot {
    fn default() -> Self {
        Self::new()
    }
}

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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::config::Config;

    // Send a test message
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