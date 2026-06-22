// SPDX-License-Identifier: GPL-3.0-only

use super::{CommandError, CommandHandler, CommandInfo, CommandMetadata, CommandContext};
use async_trait::async_trait;

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
