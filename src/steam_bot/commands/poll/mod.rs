// SPDX-License-Identifier: GPL-3.0-only

mod parse;

use super::{CommandContext, CommandError, CommandHandler, CommandInfo, CommandMetadata};
use crate::steam_bot::messaging::MessageSender;
use async_trait::async_trait;
use parse::{parse_poll_args, PollKind};

const THUMBS_UP_REACTION: &str = ":steamthumbsup:";
const THUMBS_DOWN_REACTION: &str = ":steamthumbsdown:";
const OPTION_REACTION: &str = ":steamthis:";
const POLL_USAGE: &str = "!poll <question> | !poll <question> -o <opt1> -o <opt2> [...]";

/// Poll command handler.
///
/// Posts a yes/no or multi-choice poll in the same chat where the command was used.
struct PollCommand;

impl PollCommand {
    async fn execute_poll(ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        let poll_kind = parse_poll_args(ctx.args).map_err(CommandError::InvalidArguments)?;

        match poll_kind {
            PollKind::YesNo { question } => {
                let preprocessed = MessageSender::send_to_chat_global_with_preprocessed(
                    &question,
                    ctx.chat_group_id,
                    ctx.chat_id,
                )
                .await
                .map_err(|e| CommandError::ServerError(format!("Failed to post poll: {}", e)))?;

                for reaction in [THUMBS_UP_REACTION, THUMBS_DOWN_REACTION] {
                    if let Err(e) = MessageSender::add_emoticon_reaction_from_preprocessed_global(
                        ctx.chat_group_id,
                        ctx.chat_id,
                        &preprocessed,
                        reaction,
                    )
                    .await
                    {
                        eprintln!(
                            "Failed to add poll reaction '{}' on yes/no question: {}",
                            reaction, e
                        );
                    }
                }
            }
            PollKind::Malformed { question, option } => {
                MessageSender::send_to_chat_global(&question, ctx.chat_group_id, ctx.chat_id)
                    .await
                    .map_err(|e| {
                        CommandError::ServerError(format!(
                            "Failed to post malformed poll question: {}",
                            e
                        ))
                    })?;
                MessageSender::send_to_chat_global(&option, ctx.chat_group_id, ctx.chat_id)
                    .await
                    .map_err(|e| {
                        CommandError::ServerError(format!(
                            "Failed to post malformed poll option: {}",
                            e
                        ))
                    })?;
            }
            PollKind::Multi { question, options } => {
                MessageSender::send_to_chat_global(&question, ctx.chat_group_id, ctx.chat_id)
                    .await
                    .map_err(|e| CommandError::ServerError(format!("Failed to post poll: {}", e)))?;

                for option in options {
                    let preprocessed = MessageSender::send_to_chat_global_with_preprocessed(
                        &option,
                        ctx.chat_group_id,
                        ctx.chat_id,
                    )
                    .await
                    .map_err(|e| {
                        CommandError::ServerError(format!(
                            "Failed to post poll option '{}': {}",
                            option, e
                        ))
                    })?;

                    if let Err(e) = MessageSender::add_emoticon_reaction_from_preprocessed_global(
                        ctx.chat_group_id,
                        ctx.chat_id,
                        &preprocessed,
                        OPTION_REACTION,
                    )
                    .await
                    {
                        eprintln!(
                            "Failed to add '{}' reaction on poll option '{}': {}",
                            OPTION_REACTION, option, e
                        );
                    }
                }
            }
        }

        // Poll command sends messages directly; no extra listener response required.
        Ok(String::new())
    }
}

#[async_trait]
impl CommandHandler for PollCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        Self::execute_poll(ctx).await
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "poll",
            aliases: &["q"],
            description: "Creates yes/no or multi-choice polls in the current chat.",
            usage: Some(POLL_USAGE),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "poll",
        &["q"],
        "Creates yes/no or multi-choice polls in the current chat.",
        Some(POLL_USAGE),
        || Box::new(PollCommand)
    )
}
