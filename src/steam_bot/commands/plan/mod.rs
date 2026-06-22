// SPDX-License-Identifier: GPL-3.0-only

mod mention;
mod messages;
mod time;

#[cfg(feature = "server_query")]
use crate::LiveServerInfo;
use crate::steam_bot::registry;
use super::{CommandError, CommandHandler, CommandInfo, CommandMetadata, CommandContext};
use async_trait::async_trait;
use mention::plan_actor_mention;
use messages::{format_already_planned, format_clear_response, format_plan_response, ClearResult, PlanMessageContext};
use time::{parse_time_string, time_to_unix_timestamp, unix_timestamp_to_cet_time};

/// Plan command handler
///
/// Converts a time string to Unix timestamp and outputs it to console and chat.
struct PlanCommand;

#[derive(Debug)]
struct PlanArgs<'a> {
    time_str: &'a str,
    server_id: Option<u8>,
    clear: bool,
}

#[async_trait]
impl CommandHandler for PlanCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        Ok(Self::execute_plan(ctx.args, Some(ctx.sender_id)).await)
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "plan",
            aliases: &["p"],
            description: "Sets a L4D2 server lobby plan for a specific time. Formats: 19:00, 18.30, or 16 (CET/CEST)",
            usage: Some("!plan <time> [1|2] | !plan clear [1|2]"),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "plan",
        &["p"],
        "Sets a L4D2 server lobby plan for a specific time. Formats: 19:00, 18.30, or 16 (CET/CEST)",
        Some("!plan <time> [1|2] | !plan clear [1|2]"),
        || Box::new(PlanCommand)
    )
}

impl PlanCommand {
    fn usage_hint() -> &'static str {
        "!plan <time> [1|2] | !plan clear [1|2]"
    }

    fn parse_args(args: &str) -> Result<PlanArgs<'_>, String> {
        let trimmed = args.trim();
        if trimmed.is_empty() {
            return Err(format!(
                "Usage: {} (e.g., !plan 19:00, !plan 19:00 2, !plan clear, !plan clear 1)",
                Self::usage_hint()
            ));
        }

        let parts: Vec<&str> = trimmed.split_whitespace().collect();
        match parts.as_slice() {
            [c] if c.eq_ignore_ascii_case("clear") || c.eq_ignore_ascii_case("c") || *c == "-1" => {
                Ok(PlanArgs {
                    time_str: "",
                    server_id: None,
                    clear: true,
                })
            }
            [c, sid] if c.eq_ignore_ascii_case("clear") || c.eq_ignore_ascii_case("c") => {
                let server_id = match *sid {
                    "1" => 1u8,
                    "2" => 2u8,
                    _ => {
                        return Err(format!(
                            "Invalid server id after clear. Usage: {}",
                            Self::usage_hint()
                        ));
                    }
                };
                Ok(PlanArgs {
                    time_str: "",
                    server_id: Some(server_id),
                    clear: true,
                })
            }
            [time_str] => Ok(PlanArgs {
                time_str,
                server_id: None,
                clear: false,
            }),
            [time_str, "1"] => Ok(PlanArgs {
                time_str,
                server_id: Some(1),
                clear: false,
            }),
            [time_str, "2"] => Ok(PlanArgs {
                time_str,
                server_id: Some(2),
                clear: false,
            }),
            [time_str, _] => {
                if parse_time_string(time_str).is_ok() {
                    Err(format!("Invalid server id. Usage: {}", Self::usage_hint()))
                } else {
                    Err(
                        "Invalid time format. Use HH:MM, HH.MM, or HH (e.g., 19:00, 18.30, or 16)"
                            .to_string(),
                    )
                }
            }
            _ => Err(format!("Invalid arguments. Usage: {}", Self::usage_hint())),
        }
    }

    fn selected_server_endpoint(server_id: u8) -> Result<(String, u16), String> {
        let (ip, port) = registry::config()
            .server_by_id(server_id)
            .map(|(ip, port)| (ip.to_string(), port))
            .ok_or_else(|| format!("Server {} is not configured.", server_id))?;

        ip.parse::<std::net::IpAddr>()
            .map_err(|_| format!("Server {} has an invalid IP configuration.", server_id))?;

        Ok((ip, port))
    }

    #[cfg(feature = "server_query")]
    async fn query_targeted_server_name(server_ip: &str, server_port: u16) -> Option<String> {
        match tokio::time::timeout(
            tokio::time::Duration::from_millis(300),
            LiveServerInfo::query_server_name(server_ip, server_port),
        )
        .await
        {
            Ok(Ok(server_name)) => Some(server_name),
            Ok(Err(_)) | Err(_) => None,
        }
    }

    #[cfg(not(feature = "server_query"))]
    async fn query_targeted_server_name(_server_ip: &str, _server_port: u16) -> Option<String> {
        None
    }

    async fn execute_plan(args: &str, actor: Option<u64>) -> String {
        let parsed_args = match Self::parse_args(args) {
            Ok(parsed_args) => parsed_args,
            Err(error) => return error,
        };

        if parsed_args.clear {
            return Self::execute_clear(parsed_args, actor).await;
        }

        let (hours, minutes) = match parse_time_string(parsed_args.time_str) {
            Ok(parsed) => parsed,
            Err(e) => return e,
        };

        let timestamp = match time_to_unix_timestamp(hours, minutes) {
            Ok(ts) => ts,
            Err(e) => return format!("Failed to convert time to Unix timestamp: {}", e),
        };

        println!(
            "Plan command: {}:{} → Unix timestamp: {}",
            hours, minutes, timestamp
        );

        let targeted = match parsed_args.server_id {
            Some(server_id) => match Self::selected_server_endpoint(server_id) {
                Ok((ip, port)) => Some((server_id, ip, port)),
                Err(error) => return error,
            },
            None => None,
        };

        #[cfg(feature = "rest_api")]
        {
            let existing_timestamp = match &targeted {
                Some((_, ip, _)) => crate::steam_bot::plan_broadcast::get_targeted_timestamp(ip),
                None => crate::steam_bot::plan_broadcast::get_current_timestamp(),
            };
            if existing_timestamp == Some(timestamp) {
                let time_str = unix_timestamp_to_cet_time(timestamp);
                return format_already_planned(&time_str);
            }
        }

        let is_replan = Self::is_replan(&targeted);

        #[cfg(feature = "rest_api")]
        Self::apply_reservation(&targeted, timestamp);

        let time_str = unix_timestamp_to_cet_time(timestamp);
        let actor_mention = plan_actor_mention(actor).await;

        let (server_id, server_ip, server_port, server_name) = match targeted {
            Some((server_id, server_ip, server_port)) => {
                let server_name =
                    Self::query_targeted_server_name(&server_ip, server_port).await;
                (
                    Some(server_id),
                    Some(server_ip),
                    Some(server_port),
                    server_name,
                )
            }
            None => (None, None, None, None),
        };

        format_plan_response(PlanMessageContext {
            time_str,
            actor_mention,
            server_id,
            server_ip,
            server_port,
            server_name,
            is_replan,
        })
    }

    async fn execute_clear(parsed_args: PlanArgs<'_>, actor: Option<u64>) -> String {
        let actor_mention = plan_actor_mention(actor).await;
        let actor_mention = actor_mention.as_deref();

        #[cfg(feature = "rest_api")]
        {
            if let Some(server_id) = parsed_args.server_id {
                let (ip, _) = match Self::selected_server_endpoint(server_id) {
                    Ok(endpoint) => endpoint,
                    Err(error) => return error,
                };
                let removed =
                    crate::steam_bot::plan_broadcast::clear_targeted_reservation_for_ip(&ip);
                let result = if removed {
                    ClearResult::TargetedCleared { server_id }
                } else {
                    ClearResult::TargetedNotFound {
                        server_id,
                        global_active: crate::steam_bot::plan_broadcast::get_current_timestamp()
                            .is_some(),
                    }
                };
                return format_clear_response(actor_mention, result);
            }

            crate::steam_bot::plan_broadcast::clear_reservation();
        }

        format_clear_response(actor_mention, ClearResult::GlobalCleared)
    }

    fn is_replan(targeted: &Option<(u8, String, u16)>) -> bool {
        #[cfg(feature = "rest_api")]
        {
            match targeted {
                Some((_, ip, _)) => {
                    crate::steam_bot::plan_broadcast::get_targeted_timestamp(ip).is_some()
                        || crate::steam_bot::plan_broadcast::get_current_timestamp().is_some()
                }
                None => {
                    crate::steam_bot::plan_broadcast::get_current_timestamp().is_some()
                        || crate::steam_bot::plan_broadcast::has_any_targeted_timestamp()
                }
            }
        }
        #[cfg(not(feature = "rest_api"))]
        {
            let _ = targeted;
            false
        }
    }

    #[cfg(feature = "rest_api")]
    fn apply_reservation(targeted: &Option<(u8, String, u16)>, timestamp: i64) {
        match targeted {
            Some((_, ip, _)) => {
                crate::steam_bot::plan_broadcast::set_targeted_reservation_timestamp(ip, timestamp);
            }
            None => {
                crate::steam_bot::plan_broadcast::set_reservation_timestamp(timestamp);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::PlanCommand;

    #[test]
    fn test_plan_args_invalid_second_token_after_bad_time() {
        let error = PlanCommand::parse_args("jeszcze jak").unwrap_err();
        assert!(error.contains("Invalid time format"));
    }

    #[test]
    fn test_plan_args_invalid_server_id_after_good_time() {
        let error = PlanCommand::parse_args("18:30 abc").unwrap_err();
        assert!(error.contains("Invalid server id"));
    }

    #[test]
    fn test_plan_args_clear_full() {
        let args = PlanCommand::parse_args("clear").unwrap();
        assert!(args.clear);
        assert!(args.server_id.is_none());
    }

    #[test]
    fn test_plan_args_clear_server_1() {
        let args = PlanCommand::parse_args("clear 1").unwrap();
        assert!(args.clear);
        assert_eq!(args.server_id, Some(1));
    }

    #[test]
    fn test_plan_args_clear_server_case_insensitive() {
        let args = PlanCommand::parse_args("CLEAR 2").unwrap();
        assert!(args.clear);
        assert_eq!(args.server_id, Some(2));
    }

    #[test]
    fn test_plan_args_clear_invalid_server_id() {
        let err = PlanCommand::parse_args("clear 9").unwrap_err();
        assert!(err.contains("Invalid server id after clear"));
    }
}
