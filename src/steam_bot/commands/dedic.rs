// SPDX-License-Identifier: GPL-3.0-only

use super::{CommandContext, CommandError, CommandHandler, CommandInfo, CommandMetadata};
use crate::steam_bot::registry;
use async_trait::async_trait;

const DEDIC_USAGE: &str = "!dedic [1|2] | !d [1|2]";
const DEDIC_DESCRIPTION: &str =
    "Prints the mm_dedicated_force_servers console command for the selected server (default: 1).";

/// Prints a ready-to-paste `mm_dedicated_force_servers <ip:port>` console command.
struct DedicCommand;

impl DedicCommand {
    /// Parses the optional server selector; no argument means the primary server.
    fn parse_server_id(args: &str) -> Result<u8, CommandError> {
        let parts: Vec<&str> = args.split_whitespace().collect();
        match parts.as_slice() {
            [] => Ok(1),
            ["1"] => Ok(1),
            ["2"] => Ok(2),
            _ => Err(CommandError::InvalidArguments(format!(
                "Usage: {}",
                DEDIC_USAGE
            ))),
        }
    }

    /// Picks the configured endpoint for the given server number.
    fn select_server(
        config: &crate::config::Config,
        server_id: u8,
    ) -> Result<(String, u16), CommandError> {
        let server = match server_id {
            1 => config.primary_server(),
            _ => config.secondary_server(),
        };
        server
            .map(|(ip, port)| (ip.trim().to_string(), port))
            .ok_or_else(|| {
                CommandError::ConfigError(format!("Server {} is not configured", server_id))
            })
    }

    /// Builds the reply as a `/pre` slash command. Steam's chat server (not the client) turns
    /// `/pre <text>` into a `[pre]` block; raw `[pre]...[/pre]` sent by us is not rendered.
    fn format_reply(ip: &str, port: u16) -> String {
        format!("/pre mm_dedicated_force_servers {}:{}", ip, port)
    }
}

#[async_trait]
impl CommandHandler for DedicCommand {
    async fn execute(&self, ctx: &CommandContext<'_>) -> Result<String, CommandError> {
        let server_id = Self::parse_server_id(ctx.args)?;
        let config = registry::config();
        let (ip, port) = Self::select_server(&config, server_id)?;
        Ok(Self::format_reply(&ip, port))
    }

    fn metadata(&self) -> &CommandMetadata {
        static METADATA: CommandMetadata = CommandMetadata {
            name: "dedic",
            aliases: &["d"],
            description: DEDIC_DESCRIPTION,
            usage: Some(DEDIC_USAGE),
        };
        &METADATA
    }
}

inventory::submit! {
    CommandInfo::new(
        "dedic",
        &["d"],
        DEDIC_DESCRIPTION,
        Some(DEDIC_USAGE),
        || Box::new(DedicCommand)
    )
}

#[cfg(test)]
mod tests {
    use super::DedicCommand;
    use crate::config::Config;
    use crate::steam_bot::commands::CommandError;

    fn config_with(toml_extra: &str) -> Config {
        Config::from_toml_str(&format!(
            r#"
frontend_admins = []
{}
[steam.chat]
group_id = 1234
chat_id = 5678
"#,
            toml_extra
        ))
        .expect("config")
    }

    #[test]
    fn parse_server_id_defaults_to_primary() {
        assert_eq!(DedicCommand::parse_server_id("").unwrap(), 1);
        assert_eq!(DedicCommand::parse_server_id(" 1 ").unwrap(), 1);
        assert_eq!(DedicCommand::parse_server_id("2").unwrap(), 2);
    }

    #[test]
    fn parse_server_id_rejects_other_arguments() {
        for args in ["3", "x", "1 2"] {
            match DedicCommand::parse_server_id(args) {
                Err(CommandError::InvalidArguments(msg)) => assert!(msg.contains("!dedic")),
                other => panic!("expected InvalidArguments for {:?}, got {:?}", args, other),
            }
        }
    }

    #[test]
    fn select_server_uses_configured_endpoints() {
        let config = config_with(
            "[server]\nip = \"1.2.3.4\"\nport = 27015\n[server2]\nip = \"5.6.7.8\"\nport = 27016",
        );
        assert_eq!(
            DedicCommand::select_server(&config, 1).unwrap(),
            ("1.2.3.4".to_string(), 27015)
        );
        assert_eq!(
            DedicCommand::select_server(&config, 2).unwrap(),
            ("5.6.7.8".to_string(), 27016)
        );
    }

    #[test]
    fn select_server_errors_when_not_configured() {
        let config = config_with("[server]\nip = \"1.2.3.4\"\nport = 27015");
        assert!(matches!(
            DedicCommand::select_server(&config, 2),
            Err(CommandError::ConfigError(_))
        ));
    }

    #[test]
    fn reply_is_a_pre_slash_command() {
        assert_eq!(
            DedicCommand::format_reply("1.2.3.4", 27015),
            "/pre mm_dedicated_force_servers 1.2.3.4:27015"
        );
    }
}
