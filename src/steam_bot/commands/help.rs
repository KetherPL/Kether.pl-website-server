// SPDX-License-Identifier: GPL-3.0-only

use super::{
    CommandContext, CommandError, CommandHandler, CommandInfo, CommandMetadata, CommandRegistry,
};
use async_trait::async_trait;

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

    /// Returns help for a single command name or alias.
    fn execute_command_help(command: &str) -> String {
        let registry = CommandRegistry::new();
        match registry.get_command_help(command) {
            Some((aliases, description, usage)) => {
                let mut response = format!("{}\n  {}", aliases.join(", "), description);
                if let Some(usage) = usage {
                    response.push_str(&format!("\n  Usage: {}", usage));
                }
                response
            }
            None => format!("Unknown command: {}. Use !help to list commands.", command),
        }
    }
}

#[async_trait]
impl CommandHandler for HelpCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        let command = ctx.args.split_whitespace().next().unwrap_or("");
        if command.is_empty() {
            Ok(Self::execute_help())
        } else {
            Ok(Self::execute_command_help(command))
        }
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "help",
            aliases: &["h"],
            description: "Lists all available commands.",
            usage: Some("!help [command] | !h [command]"),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "help",
        &["h"],
        "Lists all available commands.",
        Some("!help [command] | !h [command]"),
        || Box::new(HelpCommand)
    )
}
