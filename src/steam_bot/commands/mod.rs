// SPDX-License-Identifier: GPL-3.0-only

use SC_Sub_Poster::EnhancedGroupChatMessage;
use async_trait::async_trait;
use std::collections::HashMap;

use crate::steam_bot::registry;

mod help;
mod mute;
mod plan;
mod poll;
mod test;
#[cfg(feature = "server_query")]
mod status;

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
    /// Caller is not authorized to use this command
    Unauthorized(String),
    /// Command not yet implemented
    NotImplemented,
}

impl std::fmt::Display for CommandError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CommandError::InvalidArguments(msg) => write!(f, "{}", msg),
            CommandError::ServerError(msg) => write!(f, "{}", msg),
            CommandError::ConfigError(msg) => write!(f, "{}", msg),
            CommandError::Unauthorized(msg) => write!(f, "{}", msg),
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
    pub async fn handle(
        &self,
        command: &str,
        args: &str,
        message: &EnhancedGroupChatMessage,
    ) -> Result<Option<String>, CommandError> {
        let command_lower = command.to_lowercase();
        let Some(handler) = self.handlers.get(&command_lower) else {
            return Ok(None);
        };
        let ctx = CommandContext::new(args, message);
        Ok(Some(handler.execute(&ctx).await?))
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

            groups.insert(
                metadata.name.to_string(),
                (aliases, metadata.description.to_string()),
            );
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

/// Resolves the chat target for a command response.
///
/// Usage and validation errors for plan/poll stay in the invoking chat even when
/// dedicated chat routing is enabled. Successful plan responses route to the
/// dedicated planning chat when configured.
pub fn command_response_target(
    command: &str,
    result: &Result<Option<String>, CommandError>,
    source_group: u64,
    source_chat: u64,
) -> (u64, u64) {
    command_response_target_with_plan_chat(
        command,
        result,
        source_group,
        source_chat,
        registry::config().effective_plan_chat(),
    )
}

pub(crate) fn command_response_target_with_plan_chat(
    command: &str,
    result: &Result<Option<String>, CommandError>,
    source_group: u64,
    source_chat: u64,
    plan_target: Option<(u64, u64)>,
) -> (u64, u64) {
    let command_lower = command.to_lowercase();
    let is_plan = matches!(command_lower.as_str(), "plan" | "p");
    let is_poll = matches!(command_lower.as_str(), "poll" | "q");

    if result.is_err() && (is_plan || is_poll) {
        return (source_group, source_chat);
    }

    if is_plan {
        return plan_target.unwrap_or((source_group, source_chat));
    }

    (source_group, source_chat)
}

#[cfg(test)]
mod tests {
    use super::*;

    const SOURCE_GROUP: u64 = 100;
    const SOURCE_CHAT: u64 = 200;
    const DEDICATED_GROUP: u64 = 300;
    const DEDICATED_CHAT: u64 = 400;

    fn ok_response(text: &str) -> Result<Option<String>, CommandError> {
        Ok(Some(text.to_string()))
    }

    fn err_response(text: &str) -> Result<Option<String>, CommandError> {
        Err(CommandError::InvalidArguments(text.to_string()))
    }

    #[test]
    fn test_command_registry_registration() {
        let registry = CommandRegistry::new();
        let commands = registry.get_command_groups();

        // Check if basic commands are present
        let names: Vec<String> = commands
            .iter()
            .flat_map(|(aliases, _)| aliases.clone())
            .collect();
        assert!(names.contains(&"!help".to_string()) || names.contains(&"!h".to_string()));
        assert!(names.contains(&"!plan".to_string()) || names.contains(&"!p".to_string()));
    }

    #[test]
    fn plan_error_routes_to_source_chat_when_dedicated_enabled() {
        let target = command_response_target_with_plan_chat(
            "plan",
            &err_response("Usage: ..."),
            SOURCE_GROUP,
            SOURCE_CHAT,
            Some((DEDICATED_GROUP, DEDICATED_CHAT)),
        );
        assert_eq!(target, (SOURCE_GROUP, SOURCE_CHAT));
    }

    #[test]
    fn plan_success_routes_to_dedicated_chat_when_enabled() {
        let target = command_response_target_with_plan_chat(
            "p",
            &ok_response("planned lobby at 19:00"),
            SOURCE_GROUP,
            SOURCE_CHAT,
            Some((DEDICATED_GROUP, DEDICATED_CHAT)),
        );
        assert_eq!(target, (DEDICATED_GROUP, DEDICATED_CHAT));
    }

    #[test]
    fn plan_success_routes_to_source_when_dedicated_disabled() {
        let target = command_response_target_with_plan_chat(
            "plan",
            &ok_response("planned lobby at 19:00"),
            SOURCE_GROUP,
            SOURCE_CHAT,
            None,
        );
        assert_eq!(target, (SOURCE_GROUP, SOURCE_CHAT));
    }

    #[test]
    fn poll_error_routes_to_source_chat_when_dedicated_enabled() {
        let target = command_response_target_with_plan_chat(
            "q",
            &err_response("Usage: ..."),
            SOURCE_GROUP,
            SOURCE_CHAT,
            Some((DEDICATED_GROUP, DEDICATED_CHAT)),
        );
        assert_eq!(target, (SOURCE_GROUP, SOURCE_CHAT));
    }
}
