// SPDX-License-Identifier: GPL-3.0-only

use super::{CommandError, CommandHandler, CommandInfo, CommandMetadata, CommandContext};
use crate::steam_bot::registry;
use crate::LiveServerInfo;
use async_trait::async_trait;
use futures_util::future::join_all;

const SECONDS_PER_HOUR: u64 = 3600;
const SECONDS_PER_MINUTE: u64 = 60;

/// Status command handler
///
/// Queries the L4D2 server and returns a formatted status summary.
/// Only available when the `server_query` feature is enabled.
struct StatusCommand;

struct ConfiguredServer<'a> {
    id: u8,
    ip: &'a str,
    port: u16,
}

struct StatusArgs {
    server_id: Option<u8>,
    is_full: bool,
}

impl StatusCommand {
    fn usage_hint() -> &'static str {
        "!status [1|2] [full|f]"
    }

    fn configured_servers<'a>(config: &'a crate::config::Config) -> Vec<ConfiguredServer<'a>> {
        [
            config.primary_server().map(|(ip, port)| ConfiguredServer {
                id: 1,
                ip,
                port,
            }),
            config.secondary_server().map(|(ip, port)| ConfiguredServer {
                id: 2,
                ip,
                port,
            }),
        ]
        .into_iter()
        .flatten()
        .collect()
    }

    fn parse_args(args: &str) -> Result<StatusArgs, CommandError> {
        let mut server_id = None;
        let mut is_full = false;

        for token in args.split_whitespace() {
            let normalized = token.to_lowercase();

            match normalized.as_str() {
                "full" | "f" => {
                    if is_full {
                        return Err(CommandError::InvalidArguments(format!(
                            "Server Status: Invalid arguments. Usage: {}",
                            Self::usage_hint()
                        )));
                    }

                    is_full = true;
                }
                "1" | "2" => {
                    if server_id.is_some() {
                        return Err(CommandError::InvalidArguments(format!(
                            "Server Status: Invalid arguments. Usage: {}",
                            Self::usage_hint()
                        )));
                    }

                    server_id = normalized.parse().ok();
                }
                _ => {
                    return Err(CommandError::InvalidArguments(format!(
                        "Server Status: Invalid arguments. Usage: {}",
                        Self::usage_hint()
                    )));
                }
            }
        }

        Ok(StatusArgs { server_id, is_full })
    }

    fn select_servers<'a>(
        servers: Vec<ConfiguredServer<'a>>,
        server_id: Option<u8>,
    ) -> Result<Vec<ConfiguredServer<'a>>, CommandError> {
        match server_id {
            Some(server_id) => {
                let selected_servers: Vec<_> = servers
                    .into_iter()
                    .filter(|server| server.id == server_id)
                    .collect();

                if selected_servers.is_empty() {
                    Err(CommandError::ConfigError(format!(
                        "Server Status: Server {} is not configured",
                        server_id
                    )))
                } else {
                    Ok(selected_servers)
                }
            }
            None => Ok(servers),
        }
    }

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

    fn format_status_block(id: u8, body: String, multiple_servers: bool) -> String {
        if multiple_servers {
            format!("{}:\n{}", id, body)
        } else {
            body
        }
    }
}

/// Formats duration in seconds to a human-readable string
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

#[async_trait]
impl CommandHandler for StatusCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        let config = registry::config();
        let parsed_args = Self::parse_args(ctx.args)?;
        let servers = Self::select_servers(Self::configured_servers(&config), parsed_args.server_id)?;

        if servers.is_empty() {
            return Err(CommandError::ConfigError(
                "Server Status: Configuration error".to_string(),
            ));
        }

        let multiple_servers = servers.len() > 1;
        let mut responses = Vec::new();
        let mut any_server_responded = false;

        // Query all configured servers concurrently so e.g. `!s f` with two servers
        // stays within ~one query's latency instead of summing both (sequential retries).
        let query_futures = servers.iter().map(|server| {
            let ip = server.ip;
            let port = server.port;
            async move { LiveServerInfo::query_server_with_retry(ip, port, false).await }
        });
        let query_results = join_all(query_futures).await;

        for (server, query_result) in servers.iter().zip(query_results) {
            match query_result {
                Ok(server_info) => {
                    let body = if parsed_args.is_full {
                        Self::format_full_status(&server_info)
                    } else {
                        Self::format_summary_status(&server_info)
                    };

                    responses.push(Self::format_status_block(server.id, body, multiple_servers));
                    any_server_responded = true;
                }
                Err(_) if multiple_servers => {
                    responses.push(Self::format_status_block(
                        server.id,
                        "Server Status: Offline or unavailable".to_string(),
                        true,
                    ));
                }
                Err(_) => {
                    return Err(CommandError::ServerError(
                        "Server Status: Offline or unavailable".to_string(),
                    ));
                }
            }
        }

        if any_server_responded || multiple_servers {
            Ok(responses.join("\n\n"))
        } else {
            Err(CommandError::ServerError(
                "Server Status: Offline or unavailable".to_string(),
            ))
        }
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "status",
            aliases: &["s"],
            description: "Shows status for the configured server(s), optionally filtered by id.",
            usage: Some("!status | !s | !status full | !s f | !status 1 | !s 1 f | !s 2 f"),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "status",
        &["s"],
        "Shows status for the configured server(s). Use optional id 1 or 2 to select one server, and 'f' or 'full' for player lists.",
        Some("!status | !s | !status full | !s f | !status 1 | !s 1 f | !s 2 f"),
        || Box::new(StatusCommand)
    )
}
