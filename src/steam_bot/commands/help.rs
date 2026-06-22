// SPDX-License-Identifier: GPL-3.0-only

use super::{CommandError, CommandHandler, CommandInfo, CommandMetadata, CommandRegistry, CommandContext};
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
