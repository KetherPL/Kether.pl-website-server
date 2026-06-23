// SPDX-License-Identifier: GPL-3.0-only

mod mention;
mod parse;

use super::{CommandContext, CommandError, CommandHandler, CommandInfo, CommandMetadata};
use crate::steam_bot::messaging::MessageSender;
use crate::steam_bot::registry;
use async_trait::async_trait;
use mention::poll_actor_mention;
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
    fn format_question_message(question: &str, actor_mention: Option<&str>) -> String {
        match actor_mention {
            Some(actor_mention) => format!("{} ({})", question, actor_mention),
            None => question.to_string(),
        }
    }

    async fn maybe_remove_command_message(
        ctx: &CommandContext<'_>,
        should_remove: bool,
    ) {
        if !should_remove || ctx.message.timestamp == 0 {
            return;
        }

        if let Err(e) = MessageSender::delete_group_message_by_id_global(
            ctx.chat_group_id,
            ctx.chat_id,
            ctx.message.timestamp,
            ctx.message.ordinal,
        )
        .await
        {
            eprintln!("Failed to remove poll command message: {}", e);
        }
    }

    async fn execute_poll(ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        let poll_kind = parse_poll_args(ctx.args).map_err(CommandError::InvalidArguments)?;
        let config = registry::config();
        let (target_chat_group_id, target_chat_id) = config
            .effective_poll_chat()
            .unwrap_or((ctx.chat_group_id, ctx.chat_id));
        let should_remove_command_message = config.poll_chat_remove_command_message;
        let actor_mention = poll_actor_mention(Some(ctx.sender_id)).await;

        match poll_kind {
            PollKind::YesNo { question } => {
                let question_message =
                    Self::format_question_message(&question, actor_mention.as_deref());
                let preprocessed = MessageSender::send_to_chat_global_with_preprocessed(
                    &question_message,
                    target_chat_group_id,
                    target_chat_id,
                )
                .await
                .map_err(|e| CommandError::ServerError(format!("Failed to post poll: {}", e)))?;

                Self::maybe_remove_command_message(ctx, should_remove_command_message).await;

                for reaction in [THUMBS_UP_REACTION, THUMBS_DOWN_REACTION] {
                    if let Err(e) = MessageSender::add_emoticon_reaction_from_preprocessed_global(
                        target_chat_group_id,
                        target_chat_id,
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
                let question_message =
                    Self::format_question_message(&question, actor_mention.as_deref());
                MessageSender::send_to_chat_global(
                    &question_message,
                    target_chat_group_id,
                    target_chat_id,
                )
                    .await
                    .map_err(|e| {
                        CommandError::ServerError(format!(
                            "Failed to post malformed poll question: {}",
                            e
                        ))
                    })?;
                Self::maybe_remove_command_message(ctx, should_remove_command_message).await;

                MessageSender::send_to_chat_global(&option, target_chat_group_id, target_chat_id)
                    .await
                    .map_err(|e| {
                        CommandError::ServerError(format!(
                            "Failed to post malformed poll option: {}",
                            e
                        ))
                    })?;
            }
            PollKind::Multi { question, options } => {
                let question_message =
                    Self::format_question_message(&question, actor_mention.as_deref());
                MessageSender::send_to_chat_global(
                    &question_message,
                    target_chat_group_id,
                    target_chat_id,
                )
                    .await
                    .map_err(|e| CommandError::ServerError(format!("Failed to post poll: {}", e)))?;

                Self::maybe_remove_command_message(ctx, should_remove_command_message).await;

                for option in options {
                    let preprocessed = MessageSender::send_to_chat_global_with_preprocessed(
                        &option,
                        target_chat_group_id,
                        target_chat_id,
                    )
                    .await
                    .map_err(|e| {
                        CommandError::ServerError(format!(
                            "Failed to post poll option '{}': {}",
                            option, e
                        ))
                    })?;

                    if let Err(e) = MessageSender::add_emoticon_reaction_from_preprocessed_global(
                        target_chat_group_id,
                        target_chat_id,
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
