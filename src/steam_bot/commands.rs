// SPDX-License-Identifier: GPL-3.0-only

#[cfg(feature = "server_query")]
use crate::LiveServerInfo;
use crate::steam_bot::registry;
use SC_Sub_Poster::EnhancedGroupChatMessage;
use std::collections::HashMap;
use chrono::{NaiveDateTime, NaiveTime, TimeZone};
use chrono_tz::Europe::Warsaw;
use async_trait::async_trait;

// Constants for time validation and formatting
const MAX_HOURS: u8 = 23;
const MAX_MINUTES: u8 = 59;
const SECONDS_PER_HOUR: u64 = 3600;
const SECONDS_PER_MINUTE: u64 = 60;

/// Command execution context
/// 
/// Provides all necessary information for command execution including
/// the original message, parsed arguments, and chat identifiers.
#[derive(Clone)]
pub struct CommandContext<'a> {
    /// Command arguments (everything after the command name)
    pub args: &'a str,
    /// Original message that triggered the command
    pub message: &'a EnhancedGroupChatMessage,
    /// Steam ID of the message sender
    pub sender_id: u64,
    /// Chat ID where the message was sent
    pub chat_id: u64,
    /// Chat group ID where the message was sent
    pub chat_group_id: u64,
}

impl<'a> CommandContext<'a> {
    /// Creates a new command context from a message and arguments
    pub fn new(args: &'a str, message: &'a EnhancedGroupChatMessage) -> Self {
        Self {
            args,
            message,
            sender_id: u64::from(message.sender_steam_id),
            chat_id: message.chat_id,
            chat_group_id: message.chat_group_id,
        }
    }
}

/// Command execution errors
/// 
/// Structured error types for command execution failures.
/// Error messages are designed to be user-friendly and can be sent directly to chat.
#[derive(Debug)]
pub enum CommandError {
    /// Invalid command arguments with detailed error message
    InvalidArguments(String),
    /// Server query or communication error
    ServerError(String),
    /// Configuration error
    ConfigError(String),
    /// Command not yet implemented
    NotImplemented,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::InvalidArguments(msg) => write!(f, "{}", msg),
            CommandError::ServerError(msg) => write!(f, "{}", msg),
            CommandError::ConfigError(msg) => write!(f, "{}", msg),
            CommandError::NotImplemented => write!(f, "Command not implemented"),
        }
    }
}

impl std::error::Error for CommandError {}

/// Trait for parsing command arguments
/// 
/// Implement this trait for command-specific argument types to enable
/// type-safe argument parsing with validation.
pub trait CommandArgs: Sized {
    /// Parses arguments from a string
    /// 
    /// # Arguments
    /// * `args` - The argument string to parse
    /// 
    /// # Returns
    /// * `Ok(Self)` - Successfully parsed arguments
    /// * `Err(CommandError)` - Parsing or validation error
    fn parse(args: &str) -> Result<Self, CommandError>;
}

/// Command metadata
/// 
/// Contains static information about a command including its name,
/// aliases, description, and usage examples.
#[derive(Debug, Clone)]
pub struct CommandMetadata {
    /// Primary command name (e.g., "help")
    pub name: &'static str,
    /// Command aliases (e.g., ["h"])
    pub aliases: &'static [&'static str],
    /// Short description of what the command does
    pub description: &'static str,
    /// Optional usage examples (e.g., "!plan <time> | !plan clear")
    pub usage: Option<&'static str>,
}

/// Trait for command handlers
/// 
/// Commands implement this trait to handle specific command execution.
/// All commands use async execution for consistency.
#[async_trait]
pub trait CommandHandler: Send + Sync {
    /// Executes the command with the given context
    /// 
    /// # Arguments
    /// * `ctx` - The command execution context
    /// 
    /// # Returns
    /// * `Ok(String)` - The response message to send back to the chat
    /// * `Err(CommandError)` - An error that occurred during execution
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError>;
    
    /// Returns the command's metadata
    /// 
    /// # Returns
    /// A reference to the command's metadata
    fn metadata(&self) -> &CommandMetadata;
}

/// Command factory function type
/// 
/// Creates a new instance of a command handler.
type CommandFactory = fn() -> Box<dyn CommandHandler>;

/// Command registration information for inventory
/// 
/// This struct is used with the inventory crate to enable compile-time
/// command registration. Commands submit their info using `inventory::submit!`.
pub struct CommandInfo {
    /// Command metadata
    pub metadata: CommandMetadata,
    /// Factory function to create command instances
    pub factory: CommandFactory,
}

impl CommandInfo {
    /// Creates a new command info for registration
    pub const fn new(
        name: &'static str,
        aliases: &'static [&'static str],
        description: &'static str,
        usage: Option<&'static str>,
        factory: CommandFactory,
    ) -> Self {
        Self {
            metadata: CommandMetadata {
                name,
                aliases,
                description,
                usage,
            },
            factory,
        }
    }
}

inventory::collect!(CommandInfo);

/// Registry for command handlers
/// 
/// Maps command names to their handlers and provides command execution.
pub struct CommandRegistry {
    handlers: HashMap<String, Box<dyn CommandHandler>>,
}

impl CommandRegistry {
    /// Creates a new command registry with all registered commands from inventory
    pub fn new() -> Self {
        let mut handlers = HashMap::new();
        
        // Register commands from inventory
        for info in inventory::iter::<CommandInfo> {
            // Register primary name
            handlers.insert(info.metadata.name.to_lowercase(), (info.factory)());
            
            // Register aliases
            for &alias in info.metadata.aliases {
                handlers.insert(alias.to_lowercase(), (info.factory)());
            }
        }
        
        Self { handlers }
    }
    
    /// Handles a command execution
    /// 
    /// # Arguments
    /// * `command` - The command name (case-insensitive)
    /// * `args` - The command arguments
    /// * `message` - The original message
    /// 
    /// # Returns
    /// * `Ok(Some(String))` - The response message if command is found and successful
    /// * `Ok(None)` - If command is not found
    /// * `Err(CommandError)` - If command execution fails
    pub async fn handle(&self, command: &str, args: &str, message: &EnhancedGroupChatMessage) -> Result<Option<String>, CommandError> {
        let command_lower = command.to_lowercase();
        if let Some(handler) = self.handlers.get(&command_lower) {
            let ctx = CommandContext::new(args, message);
            Ok(Some(handler.execute(&ctx).await?))
        } else {
            Ok(None)
        }
    }
    
    /// Gets command groups with aliases and descriptions
    /// 
    /// Groups commands by their primary name and returns information about
    /// each command including aliases and description.
    pub fn get_command_groups(&self) -> Vec<(Vec<String>, String)> {
        let mut groups: HashMap<String, (Vec<String>, String)> = HashMap::new();
        
        // Use a set of primary names to avoid processing aliases twice
        for info in inventory::iter::<CommandInfo> {
            let metadata = &info.metadata;
            let mut aliases = vec![format!("!{}", metadata.name)];
            for &alias in metadata.aliases {
                aliases.push(format!("!{}", alias));
            }
            aliases.sort();
            
            groups.insert(metadata.name.to_string(), (aliases, metadata.description.to_string()));
        }
        
        let mut result: Vec<(Vec<String>, String)> = groups.into_values().collect();
        result.sort_by(|a, b| a.0[0].cmp(&b.0[0]));
        result
    }
    
    /// Gets a list of all registered command names (excluding test command)
    pub fn get_command_names(&self) -> Vec<String> {
        let mut commands: Vec<String> = Vec::new();
        for info in inventory::iter::<CommandInfo> {
            if info.metadata.name != "test" {
                commands.push(info.metadata.name.to_string());
                for &alias in info.metadata.aliases {
                    commands.push(alias.to_string());
                }
            }
        }
        commands.sort();
        commands
    }
}

/// Test command handler
/// 
/// Responds with "Test successful!" when executed.
struct TestCommand;

#[async_trait]
impl CommandHandler for TestCommand {
    async fn execute(&self, _ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        Ok("Test successful!".to_string())
    }
    
    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "test",
            aliases: &[],
            description: "Test command",
            usage: None,
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "test",
        &[],
        "Test command",
        None,
        || Box::new(TestCommand)
    )
}

/// Help command handler
/// 
/// Lists all available commands (excluding test command) with aliases and descriptions.
struct HelpCommand;

impl HelpCommand {
    /// Internal implementation of the help command logic
    fn execute_help() -> String {
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
}

#[async_trait]
impl CommandHandler for HelpCommand {
    async fn execute(&self, _ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        Ok(Self::execute_help())
    }
    
    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "help",
            aliases: &["h"],
            description: "Lists all available commands.",
            usage: Some("!help | !h"),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "help",
        &["h"],
        "Lists all available commands.",
        Some("!help | !h"),
        || Box::new(HelpCommand)
    )
}

/// Status command handler
/// 
/// Queries the L4D2 server and returns a formatted status summary.
/// Only available when the `server_query` feature is enabled.
#[cfg(feature = "server_query")]
struct StatusCommand;

#[cfg(feature = "server_query")]
impl StatusCommand {
    /// Formats server status as a summary (without player list)
    fn format_summary_status(server_info: &crate::LiveServerInfo::L4D2ServerInfo) -> String {
        format!(
            "Server: {}\nMap: {}\nPlayers: {}/{} (Bots: {})",
            server_info.name,
            server_info.map,
            server_info.players,
            server_info.maxplayers,
            server_info.bots
        )
    }
    
    /// Formats server status with full player list
    fn format_full_status(server_info: &crate::LiveServerInfo::L4D2ServerInfo) -> String {
        let mut response = format!(
            "Server: {}\nMap: {}\nPlayers: {}/{} (Bots: {})\n",
            server_info.name,
            server_info.map,
            server_info.players,
            server_info.maxplayers,
            server_info.bots
        );
        
        if !server_info.playerdetails.is_empty() {
            for player in &server_info.playerdetails {
                let duration_str = format_duration(player.duration);
                response.push_str(&format!("  • {} ({})\n", player.name, duration_str));
            }
            // Remove trailing newline
            response.pop();
        }
        
        response
    }
}

#[cfg(feature = "server_query")]
#[async_trait]
impl CommandHandler for StatusCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        // Get server configuration from registry
        let config = registry::config();
        let server_ip = config.server_ip().to_string();
        let server_port = config.server_port();
        
        // Check if server is configured
        if server_ip.is_empty() || server_port == 0 {
            return Err(CommandError::ConfigError("Server Status: Configuration error".to_string()));
        }
        
        // Check if "full" argument is provided
        let is_full = ctx.args.to_lowercase().trim() == "full" || ctx.args.to_lowercase().trim() == "f";
        
        // Query the server asynchronously
        match LiveServerInfo::query_server_with_retry(&server_ip, server_port).await {
            Ok(server_info) => {
                if is_full {
                    Ok(Self::format_full_status(&server_info))
                } else {
                    Ok(Self::format_summary_status(&server_info))
                }
            }
            Err(_) => {
                Err(CommandError::ServerError("Server Status: Offline or unavailable".to_string()))
            }
        }
    }
    
    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "status",
            aliases: &["s"],
            description: "Shows server status.",
            usage: Some("!status | !s | !status full | !s f"),
        };
        &METADATA
    }
}

#[cfg(feature = "server_query")]
inventory::submit! {
    CommandInfo::new(
        "status",
        &["s"],
        "Shows server status. Use 'f' or 'full' argument to see player list with play times.",
        Some("!status | !s | !status full | !s f"),
        || Box::new(StatusCommand)
    )
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
    let hours = total_seconds / SECONDS_PER_HOUR;
    let minutes = (total_seconds % SECONDS_PER_HOUR) / SECONDS_PER_MINUTE;
    let secs = total_seconds % SECONDS_PER_MINUTE;
    
    if hours > 0 {
        format!("{}h {}m", hours, minutes)
    } else if minutes > 0 {
        format!("{}m {}s", minutes, secs)
    } else {
        format!("{}s", secs)
    }
}

/// Validates hours and minutes are within valid ranges
/// 
/// # Arguments
/// * `hours` - Hours to validate
/// * `minutes` - Minutes to validate
/// 
/// # Returns
/// * `Ok(())` - If validation passes
/// * `Err(String)` - Error message if validation fails
fn validate_time_components(hours: u8, minutes: u8) -> Result<(), String> {
    if hours > MAX_HOURS {
        return Err(format!("Hours must be between 0 and {}", MAX_HOURS));
    }
    if minutes > MAX_MINUTES {
        return Err(format!("Minutes must be between 0 and {}", MAX_MINUTES));
    }
    Ok(())
}

/// Parses time string in colon format (HH:MM)
/// 
/// # Arguments
/// * `trimmed` - The trimmed time string
/// 
/// # Returns
/// * `Ok(Some((hours, minutes)))` - If colon format is found and parsed successfully
/// * `Ok(None)` - If colon format is not found
/// * `Err(String)` - If parsing fails
fn parse_colon_format(trimmed: &str) -> Result<Option<(u8, u8)>, String> {
    let colon_pos = match trimmed.find(':') {
        Some(pos) => pos,
        None => return Ok(None),
    };
    
    let hours_str = &trimmed[..colon_pos];
    let after_colon = &trimmed[colon_pos + 1..];
    // Take only the first two parts (HH:MM), ignore seconds if present
    let minutes_str = if let Some(second_colon_pos) = after_colon.find(':') {
        &after_colon[..second_colon_pos]
    } else {
        after_colon
    };
    
    let hours: u8 = hours_str.parse()
        .map_err(|_| format!("Invalid hours: {}", hours_str))?;
    let minutes: u8 = minutes_str.parse()
        .map_err(|_| format!("Invalid minutes: {}", minutes_str))?;
    
    validate_time_components(hours, minutes)?;
    Ok(Some((hours, minutes)))
}

/// Parses time string in dot format (HH.MM)
/// 
/// # Arguments
/// * `trimmed` - The trimmed time string
/// 
/// # Returns
/// * `Ok(Some((hours, minutes)))` - If dot format is found and parsed successfully
/// * `Ok(None)` - If dot format is not found
/// * `Err(String)` - If parsing fails
fn parse_dot_format(trimmed: &str) -> Result<Option<(u8, u8)>, String> {
    let dot_pos = match trimmed.find('.') {
        Some(pos) => pos,
        None => return Ok(None),
    };
    
    let hours_str = &trimmed[..dot_pos];
    let after_dot = &trimmed[dot_pos + 1..];
    // Take only the first two parts (HH.MM), ignore seconds if present
    let minutes_str = if let Some(second_dot_pos) = after_dot.find('.') {
        &after_dot[..second_dot_pos]
    } else {
        after_dot
    };
    
    let hours: u8 = hours_str.parse()
        .map_err(|_| format!("Invalid hours: {}", hours_str))?;
    let minutes: u8 = minutes_str.parse()
        .map_err(|_| format!("Invalid minutes: {}", minutes_str))?;
    
    validate_time_components(hours, minutes)?;
    Ok(Some((hours, minutes)))
}

/// Parses time string in hour-only format (HH)
/// 
/// # Arguments
/// * `trimmed` - The trimmed time string
/// 
/// # Returns
/// * `Ok((hours, 0))` - If hour-only format is parsed successfully
/// * `Err(String)` - If parsing fails
fn parse_hour_only(trimmed: &str) -> Result<(u8, u8), String> {
    let hours: u8 = trimmed.parse()
        .map_err(|_| format!("Invalid time format. Use HH:MM, HH.MM, or HH (e.g., 19:00, 18.30, or 16)"))?;
    
    validate_time_components(hours, 0)?;
    Ok((hours, 0))
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
    if let Some(result) = parse_colon_format(trimmed)? {
        return Ok(result);
    }
    
    // Try dot format (HH.MM)
    if let Some(result) = parse_dot_format(trimmed)? {
        return Ok(result);
    }
    
    // Try hour-only format (HH)
    parse_hour_only(trimmed)
}

/// Converts Unix timestamp to CET/CEST time format (HH:MM)
/// 
/// # Arguments
/// * `timestamp` - Unix timestamp in seconds
/// 
/// # Returns
/// * `String` - Time formatted as "HH:MM" in CET/CEST timezone
fn unix_timestamp_to_cet_time(timestamp: i64) -> String {
    // Convert Unix timestamp to DateTime<Utc>
    let dt_utc = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0)
        .expect("Invalid timestamp");
    
    // Convert to Warsaw timezone (CET/CEST)
    let dt_cet = dt_utc.with_timezone(&Warsaw);
    
    // Format as "HH:MM"
    dt_cet.format("%H:%M").to_string()
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

#[async_trait]
impl CommandHandler for PlanCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        Ok(Self::execute_plan(ctx.args))
    }
    
    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "plan",
            aliases: &["p"],
            description: "Converts time to Unix timestamp. Formats: 19:00, 18.30, or 16 (CET/CEST)",
            usage: Some("!plan <time> | !plan clear"),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "plan",
        &["p"],
        "Converts time to Unix timestamp. Formats: 19:00, 18.30, or 16 (CET/CEST)",
        Some("!plan <time> | !plan clear"),
        || Box::new(PlanCommand)
    )
}

impl PlanCommand {
    /// Internal implementation of the plan command logic
    /// 
    /// This method is shared between execute() and execute_async_owned() to avoid code duplication.
    fn execute_plan(args: &str) -> String {
        // Check if time argument is provided
        let time_str = args.trim();
        if time_str.is_empty() {
            return "Usage: !plan <time> (e.g., !plan 19:00, !plan 18.30, or !plan 16) or !plan clear".to_string();
        }
        
        // Check for clear command
        if time_str.eq_ignore_ascii_case("clear") || time_str == "-1" {
            #[cfg(feature = "rest_api")]
            crate::steam_bot::plan_broadcast::clear_reservation();
            return "Reservation cleared.".to_string();
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
        
        // Check for existing reservation before setting new one
        let is_replan = {
            #[cfg(feature = "rest_api")]
            {
                crate::steam_bot::plan_broadcast::get_current_timestamp().is_some()
            }
            #[cfg(not(feature = "rest_api"))]
            {
                false
            }
        };
        
        // Set reservation timestamp (this also broadcasts "SET <timestamp>" and starts expiration checker)
        #[cfg(feature = "rest_api")]
        crate::steam_bot::plan_broadcast::set_reservation_timestamp(timestamp);
        
        // Convert to CET time format
        let time_str = unix_timestamp_to_cet_time(timestamp);
        
        // Return formatted message
        if is_replan {
            format!("Re-planned lobby time: {}", time_str)
        } else {
            format!("Planned lobby time: {}", time_str)
        }
    }
}


#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_time_string_colon() {
        assert_eq!(parse_time_string("19:00").unwrap(), (19, 0));
        assert_eq!(parse_time_string("08:30").unwrap(), (8, 30));
        assert_eq!(parse_time_string("0:0").unwrap(), (0, 0));
        assert_eq!(parse_time_string("23:59").unwrap(), (23, 59));
    }

    #[test]
    fn test_parse_time_string_dot() {
        assert_eq!(parse_time_string("18.30").unwrap(), (18, 30));
        assert_eq!(parse_time_string("7.45").unwrap(), (7, 45));
    }

    #[test]
    fn test_parse_time_string_hour_only() {
        assert_eq!(parse_time_string("16").unwrap(), (16, 0));
        assert_eq!(parse_time_string("9").unwrap(), (9, 0));
    }

    #[test]
    fn test_parse_time_string_invalid() {
        assert!(parse_time_string("24:00").is_err());
        assert!(parse_time_string("12:60").is_err());
        assert!(parse_time_string("abc").is_err());
        assert!(parse_time_string("").is_err());
    }

    #[test]
    fn test_command_registry_registration() {
        let registry = CommandRegistry::new();
        let commands = registry.get_command_groups();
        
        // Check if basic commands are present
        let names: Vec<String> = commands.iter().flat_map(|(aliases, _)| aliases.clone()).collect();
        assert!(names.contains(&"!help".to_string()) || names.contains(&"!h".to_string()));
        assert!(names.contains(&"!plan".to_string()) || names.contains(&"!p".to_string()));
    }
}
