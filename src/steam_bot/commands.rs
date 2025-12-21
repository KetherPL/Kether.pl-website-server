// SPDX-License-Identifier: GPL-3.0-only

#[cfg(feature = "server_query")]
use crate::LiveServerInfo;
use crate::steam_bot::registry;
use SC_Sub_Poster::EnhancedGroupChatMessage;
use std::collections::HashMap;

/// Trait for command handlers
/// 
/// Commands implement this trait to handle specific command execution.
pub trait CommandHandler: Send + Sync {
    /// Executes the command with the given arguments
    /// 
    /// # Arguments
    /// * `args` - The command arguments (everything after the command name)
    /// * `message` - The original message that triggered the command
    /// 
    /// # Returns
    /// The response message to send back to the chat
    fn execute(&self, args: &str, message: &EnhancedGroupChatMessage) -> String;
    
    /// Executes the command asynchronously (optional, for async commands)
    /// 
    /// By default, this calls the synchronous `execute()` method.
    /// Commands that need async operations should override this method.
    /// 
    /// # Arguments
    /// * `args` - The command arguments (everything after the command name) - owned String
    /// * `message` - The original message that triggered the command
    /// 
    /// # Returns
    /// A future that resolves to the response message
    #[allow(dead_code)]
    #[allow(unused_variables)]
    fn execute_async<'a>(
        &'a self,
        args: &'a str,
        message: &'a EnhancedGroupChatMessage,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send + 'a>> {
        let args_owned = args.to_string();
        Box::pin(async move {
            // Create a dummy message for the default implementation
            // Commands should override execute_async_owned instead
            "Command not implemented".to_string()
        })
    }
    
    /// Executes the command asynchronously with owned data (for 'static futures)
    /// 
    /// By default, this returns a "not implemented" message.
    /// Commands that need async operations should override this method.
    /// 
    /// # Arguments
    /// * `_args` - The command arguments as an owned String
    /// 
    /// # Returns
    /// A 'static future that resolves to the response message
    fn execute_async_owned(
        &self,
        _args: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
        Box::pin(async move {
            // Default implementation - commands should override this
            "Command not implemented".to_string()
        })
    }
}

/// Registry for command handlers
/// 
/// Maps command names to their handlers and provides command execution.
pub struct CommandRegistry {
    handlers: HashMap<String, Box<dyn CommandHandler>>,
}

impl CommandRegistry {
    /// Creates a new command registry with all registered commands
    pub fn new() -> Self {
        let mut registry = Self {
            handlers: HashMap::new(),
        };
        
        // Register the test command
        registry.register("test", Box::new(TestCommand));
        
        // Register the status command (only if server_query feature is enabled)
        #[cfg(feature = "server_query")]
        registry.register("s", Box::new(StatusCommand));
        #[cfg(feature = "server_query")]
        registry.register("status", Box::new(StatusCommand));
        
        // Register the help command
        registry.register("h", Box::new(HelpCommand));
        registry.register("help", Box::new(HelpCommand));
        
        registry
    }
    
    /// Registers a command handler
    /// 
    /// # Arguments
    /// * `name` - The command name (case-insensitive)
    /// * `handler` - The command handler implementation
    fn register(&mut self, name: &str, handler: Box<dyn CommandHandler>) {
        self.handlers.insert(name.to_lowercase(), handler);
    }
    
    /// Handles a command execution
    /// 
    /// # Arguments
    /// * `command` - The command name (case-insensitive)
    /// * `args` - The command arguments
    /// * `message` - The original message
    /// 
    /// # Returns
    /// * `Some(String)` - The response message if command is found
    /// * `None` - If command is not found
    pub fn handle(&self, command: &str, args: &str, message: &EnhancedGroupChatMessage) -> Option<String> {
        let command_lower = command.to_lowercase();
        if let Some(handler) = self.handlers.get(&command_lower) {
            Some(handler.execute(args, message))
        } else {
            None
        }
    }
    
    /// Handles a command execution asynchronously
    /// 
    /// # Arguments
    /// * `command` - The command name (case-insensitive)
    /// * `args` - The command arguments (will be cloned for async execution)
    /// * `message` - The original message (chat IDs will be extracted)
    /// 
    /// # Returns
    /// * `Some(Future<String>)` - A future that resolves to the response message if command is found
    /// * `None` - If command is not found
    pub fn handle_async(
        &self,
        command: &str,
        args: &str,
        message: &EnhancedGroupChatMessage,
    ) -> Option<std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>>> {
        let command_lower = command.to_lowercase();
        if let Some(handler) = self.handlers.get(&command_lower) {
            // Clone the args to own them for the future
            let args_owned = args.to_string();
            // Create a static future by cloning what we need
            Some(Box::pin(handler.execute_async_owned(args_owned)))
        } else {
            None
        }
    }
    
    /// Gets a list of all registered command names (excluding test command)
    /// 
    /// # Returns
    /// A vector of command names, sorted alphabetically
    pub fn get_command_names(&self) -> Vec<String> {
        let mut commands: Vec<String> = self.handlers
            .keys()
            .filter(|name| name != &"test") // Exclude test command
            .cloned()
            .collect();
        commands.sort();
        commands
    }
}

/// Test command handler
/// 
/// Responds with "Test successful!" when executed.
struct TestCommand;

impl CommandHandler for TestCommand {
    fn execute(&self, _args: &str, _message: &EnhancedGroupChatMessage) -> String {
        "Test successful!".to_string()
    }
}

/// Status command handler
/// 
/// Queries the L4D2 server and returns a formatted status summary.
/// Only available when the `server_query` feature is enabled.
#[cfg(feature = "server_query")]
struct StatusCommand;

#[cfg(feature = "server_query")]
impl CommandHandler for StatusCommand {
    fn execute(&self, _args: &str, _message: &EnhancedGroupChatMessage) -> String {
        // Synchronous execution not supported for status command
        // Use execute_async_owned instead
        "Server Status: Please wait...".to_string()
    }
    
    fn execute_async_owned(
        &self,
        args: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
        Box::pin(async move {
            // Get server configuration from registry
            let config = registry::config();
            let server_ip = config.server_ip().to_string();
            let server_port = config.server_port();
            
            // Check if server is configured
            if server_ip.is_empty() || server_port == 0 {
                eprintln!("Server configuration error: IP or port not set");
                return "Server Status: Configuration error".to_string();
            }
            
            // Check if "full" argument is provided
            let is_full = args.to_lowercase().trim() == "full" || args.to_lowercase().trim() == "f";
            
            // Query the server asynchronously
            match LiveServerInfo::query_server_with_retry(&server_ip, server_port).await {
                Ok(server_info) => {
                    if is_full {
                        // Format the response with player list
                        let mut response = format!(
                            "Server: {}\nMap: {}\nPlayers: {}/{} (Bots: {})\n",
                            server_info.name,
                            server_info.map,
                            server_info.players,
                            server_info.maxplayers,
                            server_info.bots
                        );
                        
                        if !server_info.playerdetails.is_empty() {
                            // response.push_str("Players:\n");
                            for player in &server_info.playerdetails {
                                let duration_str = format_duration(player.duration);
                                response.push_str(&format!("  • {} ({})\n", player.name, duration_str));
                            }
                            // Remove trailing newline
                            response.pop();
                        }
                        
                        response
                    } else {
                        // Format the response as a summary
                        format!(
                            "Server: {}\nMap: {}\nPlayers: {}/{} (Bots: {})",
                            server_info.name,
                            server_info.map,
                            server_info.players,
                            server_info.maxplayers,
                            server_info.bots
                        )
                    }
                }
                Err(_) => {
                    "Server Status: Offline or unavailable".to_string()
                }
            }
        })
    }
}

/// Help command handler
/// 
/// Lists all available commands (excluding test command).
struct HelpCommand;

impl CommandHandler for HelpCommand {
    fn execute(&self, _args: &str, _message: &EnhancedGroupChatMessage) -> String {
        let registry = CommandRegistry::new();
        let commands = registry.get_command_names();
        
        if commands.is_empty() {
            "No commands available.".to_string()
        } else {
            let mut response = "Available commands:\n".to_string();
            for cmd in commands {
                response.push_str(&format!("  • !{}\n", cmd));
            }
            response.pop(); // Remove trailing newline
            response
        }
    }
    
    fn execute_async_owned(
        &self,
        _args: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
        // Help command is synchronous, so we just return the result immediately
        Box::pin(async move {
            let registry = CommandRegistry::new();
            let commands = registry.get_command_names();
            
            if commands.is_empty() {
                "No commands available.".to_string()
            } else {
                let mut response = "Available commands:\n".to_string();
                for cmd in commands {
                    response.push_str(&format!("  • !{}\n", cmd));
                }
                response.pop(); // Remove trailing newline
                response
            }
        })
    }
}

/// Formats duration in seconds to a human-readable string
/// 
/// # Arguments
/// * `seconds` - Duration in seconds as f32
/// 
/// # Returns
/// Formatted string like "5m 30s" or "1h 15m" or "45s"
fn format_duration(seconds: f32) -> String {
    let total_seconds = seconds as u64;
    let hours = total_seconds / 3600;
    let minutes = (total_seconds % 3600) / 60;
    let secs = total_seconds % 60;
    
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

