// SPDX-License-Identifier: GPL-3.0-only

#[cfg(feature = "server_query")]
use crate::LiveServerInfo;
use crate::steam_bot::registry;
use SC_Sub_Poster::EnhancedGroupChatMessage;
use std::collections::HashMap;
use chrono::{NaiveDateTime, NaiveTime, TimeZone};
use chrono_tz::Europe::Warsaw;

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
        
        // Register the plan command
        registry.register("p", Box::new(PlanCommand));
        registry.register("plan", Box::new(PlanCommand));
        
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
        _message: &EnhancedGroupChatMessage,
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
    
    /// Gets command groups with aliases and descriptions
    /// 
    /// Groups commands by their handler type and returns information about
    /// each command group including aliases and usage.
    /// 
    /// # Returns
    /// A vector of tuples: (aliases, description)
    pub fn get_command_groups(&self) -> Vec<(Vec<String>, String)> {
        // Manually define command groups based on known aliases
        // This is simpler than trying to use TypeId with trait objects
        let mut groups: Vec<(Vec<String>, String)> = Vec::new();
        
        // Help command group
        let mut help_aliases = Vec::new();
        if self.handlers.contains_key("help") {
            help_aliases.push("!help".to_string());
        }
        if self.handlers.contains_key("h") {
            help_aliases.push("!h".to_string());
        }
        if !help_aliases.is_empty() {
            help_aliases.sort();
            groups.push((help_aliases, "Lists all available commands.".to_string()));
        }
        
        // Status command group (only if server_query feature is enabled)
        #[cfg(feature = "server_query")]
        {
            let mut status_aliases = Vec::new();
            if self.handlers.contains_key("status") {
                status_aliases.push("!status".to_string());
            }
            if self.handlers.contains_key("s") {
                status_aliases.push("!s".to_string());
            }
            if !status_aliases.is_empty() {
                status_aliases.sort();
                groups.push((status_aliases, "Shows server status. Use 'f' or 'full' argument to see player list with play times.".to_string()));
            }
        }
        
        // Plan command group
        let mut plan_aliases = Vec::new();
        if self.handlers.contains_key("plan") {
            plan_aliases.push("!plan".to_string());
        }
        if self.handlers.contains_key("p") {
            plan_aliases.push("!p".to_string());
        }
        if !plan_aliases.is_empty() {
            plan_aliases.sort();
            groups.push((plan_aliases, "Converts time to Unix timestamp. Formats: 19:00, 18.30, or 16 (CET/CEST)".to_string()));
        }
        
        // Sort groups by first alias
        groups.sort_by(|a, b| a.0[0].cmp(&b.0[0]));
        groups
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
/// Lists all available commands (excluding test command) with aliases and descriptions.
struct HelpCommand;

impl CommandHandler for HelpCommand {
    fn execute(&self, _args: &str, _message: &EnhancedGroupChatMessage) -> String {
        let registry = CommandRegistry::new();
        let groups = registry.get_command_groups();
        
        if groups.is_empty() {
            "No commands available.".to_string()
        } else {
            let mut response = "Available commands:\n".to_string();
            for (aliases, description) in groups {
                // Join aliases with commas
                let aliases_str = aliases.join(", ");
                response.push_str(&format!("  {}\n    {}\n", aliases_str, description));
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
            let groups = registry.get_command_groups();
            
            if groups.is_empty() {
                "No commands available.".to_string()
            } else {
                let mut response = "Available commands:\n".to_string();
                for (aliases, description) in groups {
                    // Join aliases with commas
                    let aliases_str = aliases.join(", ");
                    response.push_str(&format!("  {}\n    {}\n", aliases_str, description));
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

/// Parses a time string in multiple formats (HH:MM, HH.MM, or HH)
/// 
/// # Arguments
/// * `time_str` - The time string to parse
/// 
/// # Returns
/// * `Ok((hours, minutes))` - Parsed hours and minutes
/// * `Err(String)` - Error message if parsing fails
/// 
/// # Examples
/// - "19:00" → Ok((19, 0))
/// - "18.30" → Ok((18, 30))
/// - "16" → Ok((16, 0))
fn parse_time_string(time_str: &str) -> Result<(u8, u8), String> {
    let trimmed = time_str.trim();
    
    if trimmed.is_empty() {
        return Err("Time string cannot be empty".to_string());
    }
    
    // Try colon format (HH:MM)
    // Note: If seconds are provided (HH:MM:SS), we ignore them and only use HH:MM
    if let Some(colon_pos) = trimmed.find(':') {
        let hours_str = &trimmed[..colon_pos];
        // Take only the first two parts (HH:MM), ignore seconds if present
        let after_colon = &trimmed[colon_pos + 1..];
        let minutes_str = if let Some(second_colon_pos) = after_colon.find(':') {
            &after_colon[..second_colon_pos]
        } else {
            after_colon
        };
        
        let hours: u8 = hours_str.parse()
            .map_err(|_| format!("Invalid hours: {}", hours_str))?;
        let minutes: u8 = minutes_str.parse()
            .map_err(|_| format!("Invalid minutes: {}", minutes_str))?;
        
        if hours > 23 {
            return Err("Hours must be between 0 and 23".to_string());
        }
        if minutes > 59 {
            return Err("Minutes must be between 0 and 59".to_string());
        }
        
        return Ok((hours, minutes));
    }
    
    // Try dot format (HH.MM)
    // Note: If additional dots are provided (HH.MM.SS), we ignore them and only use HH.MM
    if let Some(dot_pos) = trimmed.find('.') {
        let hours_str = &trimmed[..dot_pos];
        // Take only the first two parts (HH.MM), ignore seconds if present
        let after_dot = &trimmed[dot_pos + 1..];
        let minutes_str = if let Some(second_dot_pos) = after_dot.find('.') {
            &after_dot[..second_dot_pos]
        } else {
            after_dot
        };
        
        let hours: u8 = hours_str.parse()
            .map_err(|_| format!("Invalid hours: {}", hours_str))?;
        let minutes: u8 = minutes_str.parse()
            .map_err(|_| format!("Invalid minutes: {}", minutes_str))?;
        
        if hours > 23 {
            return Err("Hours must be between 0 and 23".to_string());
        }
        if minutes > 59 {
            return Err("Minutes must be between 0 and 59".to_string());
        }
        
        return Ok((hours, minutes));
    }
    
    // Try hour-only format (HH)
    let hours: u8 = trimmed.parse()
        .map_err(|_| format!("Invalid time format. Use HH:MM, HH.MM, or HH (e.g., 19:00, 18.30, or 16)"))?;
    
    if hours > 23 {
        return Err("Hours must be between 0 and 23".to_string());
    }
    
    Ok((hours, 0))
}

/// Converts hours and minutes to Unix timestamp using CET/CEST timezone
/// 
/// # Arguments
/// * `hours` - Hours (0-23)
/// * `minutes` - Minutes (0-59)
/// 
/// # Returns
/// * `Ok(i64)` - Unix timestamp in seconds
/// * `Err(String)` - Error message if conversion fails
/// 
/// # Note
/// During DST transitions (especially fall back), ambiguous times are resolved
/// by preferring the later occurrence (standard time).
fn time_to_unix_timestamp(hours: u8, minutes: u8) -> Result<i64, String> {
    // Get current date/time in CET/CEST
    let now_utc = chrono::Utc::now();
    let now_cet = now_utc.with_timezone(&Warsaw);
    
    // Get current date in CET/CEST
    let current_date = now_cet.date_naive();
    
    // Create NaiveTime from hours and minutes
    let time = NaiveTime::from_hms_opt(hours as u32, minutes as u32, 0)
        .ok_or_else(|| "Failed to create time from hours and minutes".to_string())?;
    
    // Combine date and time
    let naive_dt = NaiveDateTime::new(current_date, time);
    
    // Convert to timezone-aware DateTime in CET/CEST
    // During DST transitions, ambiguous times can occur (especially during fall back).
    // We use .earliest() to prefer the first occurrence, or .latest() for the later one.
    // For planning purposes, we'll use .latest() to prefer standard time during ambiguity.
    let dt_cet = match Warsaw.from_local_datetime(&naive_dt) {
        chrono::LocalResult::Single(dt) => dt,
        chrono::LocalResult::Ambiguous(_dt1, dt2) => {
            // During ambiguous time (DST fall back), prefer the later occurrence (standard time)
            dt2
        }
        chrono::LocalResult::None => {
            return Err("Time does not exist in CET/CEST timezone (invalid DST transition)".to_string());
        }
    };
    
    // Convert to Unix timestamp
    Ok(dt_cet.timestamp())
}

/// Plan command handler
/// 
/// Converts a time string to Unix timestamp and outputs it to console and chat.
struct PlanCommand;

impl CommandHandler for PlanCommand {
    fn execute(&self, args: &str, _message: &EnhancedGroupChatMessage) -> String {
        Self::execute_plan(args)
    }
    
    fn execute_async_owned(
        &self,
        args: String,
    ) -> std::pin::Pin<Box<dyn std::future::Future<Output = String> + Send>> {
        // Plan command is synchronous, so we just return the result immediately
        let result = Self::execute_plan(&args);
        Box::pin(async move {
            result
        })
    }
}

impl PlanCommand {
    /// Internal implementation of the plan command logic
    /// 
    /// This method is shared between execute() and execute_async_owned() to avoid code duplication.
    fn execute_plan(args: &str) -> String {
        // Check if time argument is provided
        let time_str = args.trim();
        if time_str.is_empty() {
            return "Usage: !plan <time> (e.g., !plan 19:00, !plan 18.30, or !plan 16)".to_string();
        }
        
        // Parse the time string
        let (hours, minutes) = match parse_time_string(time_str) {
            Ok(parsed) => parsed,
            Err(e) => return e,
        };
        
        // Convert to Unix timestamp
        let timestamp = match time_to_unix_timestamp(hours, minutes) {
            Ok(ts) => ts,
            Err(e) => return format!("Failed to convert time to Unix timestamp: {}", e),
        };
        
        // Print to console
        println!("Plan command: {}:{} → Unix timestamp: {}", hours, minutes, timestamp);
        
        // Broadcast timestamp to WebSocket clients
        #[cfg(feature = "rest_api")]
        crate::steam_bot::plan_broadcast::broadcast_timestamp(timestamp);
        
        // Return timestamp as string for chat response
        timestamp.to_string()
    }
}

