// SPDX-License-Identifier: GPL-3.0-only

use super::mention::user_mention;
use super::{CommandError, CommandHandler, CommandInfo, CommandMetadata, CommandContext};
use crate::steam_bot::registry;
use async_trait::async_trait;

const TEST_USAGE: &str = "!test [m] (m = mention yourself)";

/// Test command handler
///
/// Responds with "Test successful!"; with the `m` argument it also mentions the sender.
struct TestCommand;

impl TestCommand {
    /// `true` when the first argument is `m` (case-insensitive).
    fn wants_mention(args: &str) -> bool {
        args.split_whitespace()
            .next()
            .is_some_and(|arg| arg.eq_ignore_ascii_case("m"))
    }
}

#[async_trait]
impl CommandHandler for TestCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        if !Self::wants_mention(ctx.args) {
            return Ok("Test successful!".to_string());
        }

        let config = registry::config();
        if config.steam_web_api_key.trim().is_empty() {
            return Ok(
                "Test successful! (can't mention you: steam.web_api_key is not set)".to_string(),
            );
        }

        Ok(
            match user_mention(ctx.sender_id, &config.steam_web_api_key, "!test sender").await {
                Some(mention) => format!("Test successful! {}", mention),
                None => {
                    "Test successful! (couldn't resolve your Steam name for the mention)".to_string()
                }
            },
        )
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "test",
            aliases: &[],
            description: "Test command",
            usage: Some(TEST_USAGE),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "test",
        &[],
        "Test command",
        Some(TEST_USAGE),
        || Box::new(TestCommand)
    )
}

#[cfg(test)]
mod tests {
    use super::TestCommand;

    #[test]
    fn wants_mention_only_for_m_argument() {
        assert!(!TestCommand::wants_mention(""));
        assert!(TestCommand::wants_mention("m"));
        assert!(TestCommand::wants_mention(" M "));
        assert!(!TestCommand::wants_mention("x"));
        assert!(!TestCommand::wants_mention("mention"));
    }
}
