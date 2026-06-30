// SPDX-License-Identifier: GPL-3.0-only

use std::{fmt, path::{Path, PathBuf}};
use colored::Colorize;
use rocket::serde::{Deserialize, Serialize};
use smol::fs;

/// Configuration file name constant
pub const CONF_FILE_NAME: &str = "config.toml";

/// Main configuration structure loaded from config.toml
/// 
/// This struct holds all configuration values for the Kether Internal Services Server.
/// It uses nested structures for better organization.
#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct ConfigFile {
	/// Admin Steam IDs for frontend verification
	#[serde(default)]
	frontend_admins: Vec<i64>,

	#[serde(default)]
	steam: SteamConfig,

	#[serde(default)]
	server: ServerConfig,

	#[serde(default)]
	server2: ServerConfig,
}

/// Steam-related configuration
#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct SteamConfig {
	/// Steam Web API key for fetching user data
	#[serde(default)]
	web_api_key: String,
	
	/// Steam bot configuration
	#[serde(default)]
	bot: BotConfig,
	
	/// Steam chat configuration
	#[serde(default)]
	chat: ChatConfig,
}

/// Steam bot credentials
#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct BotConfig {
	#[serde(default)]
	username: String,
	
	#[serde(default)]
	password: String,
}

fn default_true() -> bool {
	true
}

fn default_mute_max_minutes() -> u64 {
	10080
}

/// Admin-only SteamBot command settings (e.g. !mute)
#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct AdminConfig {
	/// Steam IDs allowed to use admin-only SteamBot commands (e.g. !mute)
	#[serde(default)]
	admins: Vec<i64>,

	/// When true, SteamBot admin commands use frontend_admins instead of admins
	#[serde(default = "default_true")]
	admins_same_as_frontend: bool,

	/// Maximum mute duration in minutes (default 7 days)
	#[serde(default = "default_mute_max_minutes")]
	mute_max_minutes: u64,

	/// When true, commands from currently muted users are ignored
	#[serde(default = "default_true")]
	ignore_muted_commands: bool,
}

impl Default for AdminConfig {
	fn default() -> Self {
		AdminConfig {
			admins: Vec::new(),
			admins_same_as_frontend: true,
			mute_max_minutes: default_mute_max_minutes(),
			ignore_muted_commands: true,
		}
	}
}

/// Steam chat configuration
#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct ChatConfig {
	#[serde(default)]
	group_id: u64,
	
	#[serde(default)]
	chat_id: u64,

	/// Whether commands can be invoked without mentioning the bot
	#[serde(default)]
	commands_without_mention: bool,

	/// Dedicated chat ID for lobby planning command responses
	#[serde(default)]
	plan_chat_id: u64,

	/// Whether dedicated planning chat routing is enabled
	#[serde(default)]
	dedicated_plan_chat: bool,

	/// Whether to keep dedicated planning chat clean from non-bot messages
	#[serde(default)]
	plan_chat_keep_clean: bool,

	/// Whether to mention the user who executed !plan
	#[serde(default)]
	plan_mention_user: bool,

	/// Dedicated chat ID for poll command responses
	#[serde(default)]
	poll_chat_id: u64,

	/// Whether dedicated poll chat routing is enabled
	#[serde(default)]
	dedicated_poll_chat: bool,

	/// Whether poll command invocations should be deleted after posting poll messages
	#[serde(default)]
	poll_chat_remove_command_message: bool,

	/// Whether to mention the user who executed !poll
	#[serde(default)]
	poll_mention_user: bool,

	/// Admin-only SteamBot command settings
	#[serde(default)]
	admin: AdminConfig,
}

/// L4D2 server configuration
#[derive(Debug, Deserialize, Serialize)]
#[serde(crate = "rocket::serde")]
struct ServerConfig {
	/// IP address of the L4D2 server
	#[serde(default)]
	ip: String,
	
	/// Port number of the L4D2 server
	#[serde(default)]
	port: u16,
}

impl ServerConfig {
	fn empty() -> Self {
		ServerConfig {
			ip: String::new(),
			port: 0,
		}
	}
}

impl Default for ConfigFile {
	fn default() -> Self {
		ConfigFile {
			frontend_admins: Vec::new(),
			steam: SteamConfig::default(),
			server: ServerConfig::default(),
			server2: ServerConfig::default(),
		}
	}
}

impl Default for SteamConfig {
	fn default() -> Self {
		SteamConfig {
			web_api_key: String::new(),
			bot: BotConfig::default(),
			chat: ChatConfig::default(),
		}
	}
}

impl Default for BotConfig {
	fn default() -> Self {
		BotConfig {
			username: String::new(),
			password: String::new(),
		}
	}
}

impl Default for ChatConfig {
	fn default() -> Self {
		ChatConfig {
			group_id: 0,
			chat_id: 0,
			commands_without_mention: false,
			plan_chat_id: 0,
			dedicated_plan_chat: false,
			plan_chat_keep_clean: false,
			plan_mention_user: false,
			poll_chat_id: 0,
			dedicated_poll_chat: false,
			poll_chat_remove_command_message: false,
			poll_mention_user: false,
			admin: AdminConfig::default(),
		}
	}
}

impl Default for ServerConfig {
	fn default() -> Self {
		ServerConfig::empty()
	}
}

/// Public configuration structure for the Kether Internal Services Server
/// 
/// This struct holds all configuration values in a flat structure
/// for easy access throughout the application.
/// 
/// # Fields
/// * `frontend_admins` - List of Steam IDs with admin permissions
/// * `steam_web_api_key` - Steam Web API key for fetching user data
/// * `chat_group_id` - Steam group ID for chat functionality
/// * `chat_id` - Steam chat ID for message sending
/// * `plan_chat_id` - Dedicated chat ID for lobby planning responses
/// * `dedicated_plan_chat` - Whether dedicated planning chat routing is enabled
/// * `plan_chat_keep_clean` - Whether non-bot messages should be removed from dedicated planning chat
/// * `plan_mention_user` - Whether planning messages mention the executing user
/// * `poll_chat_id` - Dedicated chat ID for poll responses
/// * `dedicated_poll_chat` - Whether dedicated poll chat routing is enabled
/// * `poll_chat_remove_command_message` - Whether poll command messages should be removed after posting polls
/// * `poll_mention_user` - Whether poll messages mention the executing user
/// * `steam_account` - Steam account username for bot login
/// * `steam_password` - Steam account password for bot login
/// * `server_ip` - IP address of the L4D2 server
/// * `server_port` - Port number of the L4D2 server
#[derive(Debug, Clone)]
pub struct Config {
	pub frontend_admins: Vec<i64>,
	pub steam_web_api_key: String,
	pub chat_group_id: u64,
	pub chat_id: u64,
	pub plan_chat_id: u64,
	pub dedicated_plan_chat: bool,
	pub plan_chat_keep_clean: bool,
	pub plan_mention_user: bool,
	pub poll_chat_id: u64,
	pub dedicated_poll_chat: bool,
	pub poll_chat_remove_command_message: bool,
	pub poll_mention_user: bool,
	pub steam_account: String,
	pub steam_password: String,
	pub steambot_admins: Vec<i64>,
	pub steambot_admins_same_as_frontend: bool,
	pub steambot_mute_max_minutes: u64,
	pub steambot_ignore_muted_commands: bool,
	pub server_ip: String,
	pub server_port: u16,
	pub server2_ip: String,
	pub server2_port: u16,
	pub steam_bot_commands_without_mention: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct ConfigChange {
	pub live_applied: Vec<&'static str>,
	pub requires_restart: Vec<&'static str>,
	pub unchanged: bool,
}

impl ConfigChange {
	pub fn log(&self) {
		if self.unchanged {
			return;
		}

		if !self.live_applied.is_empty() {
			println!(
				"Config hot reload: applied live fields: {}",
				self.live_applied.join(", ")
			);
		}

		if !self.requires_restart.is_empty() {
			eprintln!(
				"Config hot reload: restart required for fields: {}",
				self.requires_restart.join(", ")
			);
		}
	}
}

impl Config {
	/// Loads configuration from config.toml file
	/// 
	/// This function reads the TOML configuration file and creates a Config struct.
	/// If the file doesn't exist, it creates a default one with helpful comments.
	/// 
	/// # Returns
	/// * `Ok(Config)` - Successfully loaded configuration
	/// * `Err(Box<dyn std::error::Error>)` - If loading or parsing fails
	pub async fn load() -> Result<Self, Box<dyn std::error::Error>> {
		let conf_path = exe_dir()?.join(CONF_FILE_NAME);
		
		// Create default config if it doesn't exist
		if !smol::fs::metadata(&conf_path).await.is_ok() {
			println!("Creating default config file at: {}", conf_path.display());
			let default_config = ConfigFile::default();
			let toml_content = Self::generate_toml_with_comments(&default_config);
			fs::write(&conf_path, toml_content).await?;
		}
		
		// Load and parse TOML
		let content = fs::read_to_string(&conf_path).await?;
		Self::from_toml_str(&content).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
	}

	pub fn load_from(path: &Path) -> Result<Self, Box<dyn std::error::Error>> {
		let content = std::fs::read_to_string(path)?;
		Self::from_toml_str(&content).map_err(|e| Box::new(e) as Box<dyn std::error::Error>)
	}

	pub fn from_toml_str(content: &str) -> Result<Self, toml::de::Error> {
		let config_file: ConfigFile = toml::from_str(content)?;
		Ok(Self::from_config_file(config_file))
	}

	fn from_config_file(config_file: ConfigFile) -> Self {
		Self {
			frontend_admins: config_file.frontend_admins,
			steam_web_api_key: config_file.steam.web_api_key,
			chat_group_id: config_file.steam.chat.group_id,
			chat_id: config_file.steam.chat.chat_id,
			plan_chat_id: config_file.steam.chat.plan_chat_id,
			dedicated_plan_chat: config_file.steam.chat.dedicated_plan_chat,
			plan_chat_keep_clean: config_file.steam.chat.plan_chat_keep_clean,
			plan_mention_user: config_file.steam.chat.plan_mention_user,
			poll_chat_id: config_file.steam.chat.poll_chat_id,
			dedicated_poll_chat: config_file.steam.chat.dedicated_poll_chat,
			poll_chat_remove_command_message: config_file.steam.chat.poll_chat_remove_command_message,
			poll_mention_user: config_file.steam.chat.poll_mention_user,
			steam_account: config_file.steam.bot.username,
			steam_password: config_file.steam.bot.password,
			steambot_admins: config_file.steam.chat.admin.admins,
			steambot_admins_same_as_frontend: config_file.steam.chat.admin.admins_same_as_frontend,
			steambot_mute_max_minutes: config_file.steam.chat.admin.mute_max_minutes,
			steambot_ignore_muted_commands: config_file.steam.chat.admin.ignore_muted_commands,
			server_ip: config_file.server.ip,
			server_port: config_file.server.port,
			server2_ip: config_file.server2.ip,
			server2_port: config_file.server2.port,
			steam_bot_commands_without_mention: config_file.steam.chat.commands_without_mention,
		}
	}

	pub fn diff(&self, new: &Config) -> ConfigChange {
		let mut change = ConfigChange::default();

		if self.frontend_admins != new.frontend_admins {
			change.live_applied.push("frontend_admins");
		}
		if self.steam_web_api_key != new.steam_web_api_key {
			change.live_applied.push("steam.web_api_key");
		}
		if self.server_ip != new.server_ip {
			change.live_applied.push("server.ip");
		}
		if self.server_port != new.server_port {
			change.live_applied.push("server.port");
		}
		if self.server2_ip != new.server2_ip {
			change.live_applied.push("server2.ip");
		}
		if self.server2_port != new.server2_port {
			change.live_applied.push("server2.port");
		}
		if self.steam_bot_commands_without_mention != new.steam_bot_commands_without_mention {
			change.live_applied.push("steam.chat.commands_without_mention");
		}
		if self.plan_chat_id != new.plan_chat_id {
			change.live_applied.push("steam.chat.plan_chat_id");
		}
		if self.dedicated_plan_chat != new.dedicated_plan_chat {
			change.live_applied.push("steam.chat.dedicated_plan_chat");
		}
		if self.plan_chat_keep_clean != new.plan_chat_keep_clean {
			change.live_applied.push("steam.chat.plan_chat_keep_clean");
		}
		if self.plan_mention_user != new.plan_mention_user {
			change.live_applied.push("steam.chat.plan_mention_user");
		}
		if self.poll_chat_id != new.poll_chat_id {
			change.live_applied.push("steam.chat.poll_chat_id");
		}
		if self.dedicated_poll_chat != new.dedicated_poll_chat {
			change.live_applied.push("steam.chat.dedicated_poll_chat");
		}
		if self.poll_chat_remove_command_message != new.poll_chat_remove_command_message {
			change
				.live_applied
				.push("steam.chat.poll_chat_remove_command_message");
		}
		if self.poll_mention_user != new.poll_mention_user {
			change.live_applied.push("steam.chat.poll_mention_user");
		}
		if self.steambot_admins != new.steambot_admins {
			change.live_applied.push("steam.chat.admin.admins");
		}
		if self.steambot_admins_same_as_frontend != new.steambot_admins_same_as_frontend {
			change.live_applied.push("steam.chat.admin.admins_same_as_frontend");
		}
		if self.steambot_mute_max_minutes != new.steambot_mute_max_minutes {
			change.live_applied.push("steam.chat.admin.mute_max_minutes");
		}
		if self.steambot_ignore_muted_commands != new.steambot_ignore_muted_commands {
			change.live_applied.push("steam.chat.admin.ignore_muted_commands");
		}
		if self.steam_account != new.steam_account {
			change.requires_restart.push("steam.bot.username");
		}
		if self.steam_password != new.steam_password {
			change.requires_restart.push("steam.bot.password");
		}
		if self.chat_group_id != new.chat_group_id {
			change.requires_restart.push("steam.chat.group_id");
		}
		if self.chat_id != new.chat_id {
			change.requires_restart.push("steam.chat.chat_id");
		}

		change.unchanged = change.live_applied.is_empty() && change.requires_restart.is_empty();
		change
	}
	
	/// Generate TOML content with helpful comments
	fn generate_toml_with_comments(_config: &ConfigFile) -> String {
		r#"# Kether.pl Kether Internal Services Server Configuration
# This file is automatically generated. Edit values as needed.

# Frontend Admin Steam IDs (64-bit numeric format)
# Users with these Steam IDs have admin permissions on the frontend
# Add Steam IDs here, one per line for clarity, divided by commas
frontend_admins = []

# L4D2 Server Configuration
# Server address used for !status command and server queries (main server)
[server]
ip = ""
port = 0

# Secondary L4D2 Server Configuration
# Server address used for !status command and server queries (secondary server; usually for testing purposes)
[server2]
ip = ""
port = 0

[steam]
# Steam Web API key for fetching user data (name, avatar, profile, etc.)
# Get your key from: https://steamcommunity.com/dev/apikey
web_api_key = ""

# Steam Bot Configuration
# These credentials are used for the bot that posts !sub requests to Steam group chat
[steam.bot]
username = ""
password = ""

# Steam Group Chat Configuration
# IDs for the Steam group chat where !sub requests are posted
[steam.chat]
group_id = 0
chat_id = 0

# Whether commands can be invoked without mentioning the bot (true/false)
# If true, simply typing "!plan 19" will work without @mentioning the bot
commands_without_mention = false

# Dedicated chat room id for planning messages (!plan / !plan clear)
# Uses the same steam.chat.group_id; set to 0 to disable
plan_chat_id = 0

# If true, planning command responses are posted only to plan_chat_id
# If plan_chat_id is 0, this behaves as disabled
dedicated_plan_chat = false

# If true, removes non-bot messages from the dedicated planning chat
# Works only when plan_chat_id is configured
plan_chat_keep_clean = false

# If true, planning messages mention the user who executed the command
plan_mention_user = false

# Dedicated chat room id for poll messages (!poll / !q)
# Uses the same steam.chat.group_id; set to 0 to disable
poll_chat_id = 0

# If true, poll command responses are posted only to poll_chat_id
# If poll_chat_id is 0, this behaves as disabled
dedicated_poll_chat = false

# If true, removes the invoking user command message after poll posting
poll_chat_remove_command_message = false

# If true, poll question messages mention the user who executed the command
poll_mention_user = false

# Admin-only SteamBot command settings (!mute / !unmute / !lsmute)
[steam.chat.admin]
# Steam IDs allowed to use admin-only commands (when admins_same_as_frontend = false)
admins = []
# When true, admin commands use frontend_admins instead of admins
admins_same_as_frontend = true
# Maximum mute duration in minutes (default 7 days = 10080)
mute_max_minutes = 10080
# When true, commands from muted users are ignored
ignore_muted_commands = true
"#.to_string()
	}
	
	/// Check if a Steam ID has admin permissions
	/// 
	/// # Arguments
	/// * `steam_id` - The Steam ID to check (64-bit format)
	/// 
	/// # Returns
	/// * `true` if the Steam ID is in the admin list
	/// * `false` otherwise
	pub fn is_admin(&self, steam_id: i64) -> bool {
		self.frontend_admins.contains(&steam_id)
	}

	/// Returns the Steam ID list used for SteamBot admin commands.
	pub fn steambot_admin_ids(&self) -> &[i64] {
		if self.steambot_admins_same_as_frontend {
			&self.frontend_admins
		} else {
			&self.steambot_admins
		}
	}

	/// Whether a Steam ID may use admin-only SteamBot commands (e.g. !mute).
	pub fn is_steambot_admin(&self, steam_id: u64) -> bool {
		self.steambot_admin_ids().contains(&(steam_id as i64))
	}

	/// Get the L4D2 server IP address
	/// 
	/// # Returns
	/// The server IP address as a string
	pub fn server_ip(&self) -> &str {
		&self.server_ip
	}

	/// Get the L4D2 server port number
	/// 
	/// # Returns
	/// The server port number
	pub fn server_port(&self) -> u16 {
		self.server_port
	}

	/// Get the primary L4D2 server endpoint if configured
	///
	/// # Returns
	/// `Some((ip, port))` when both values are set, otherwise `None`
	/// (For the Steam bot status command to use)
	pub fn primary_server(&self) -> Option<(&str, u16)> {
		if self.server_ip.trim().is_empty() || self.server_port == 0 {
			None
		} else {
			Some((self.server_ip(), self.server_port()))
		}
	}

	/// Get the L4D2 server2 IP address
	/// 
	/// # Returns
	/// The server2 IP address as a string
	pub fn server2_ip(&self) -> &str {
		&self.server2_ip
	}

	/// Get the L4D2 server2 port number
	/// 
	/// # Returns
	/// The server2 port number
	pub fn server2_port(&self) -> u16 {
		self.server2_port
	}

	/// Get the secondary L4D2 server endpoint if configured
	///
	/// # Returns
	/// `Some((ip, port))` when both values are set, otherwise `None`
	/// (For the Steam bot !status command to use)
	pub fn secondary_server(&self) -> Option<(&str, u16)> {
		if self.server2_ip.trim().is_empty() || self.server2_port == 0 {
			None
		} else {
			Some((self.server2_ip(), self.server2_port()))
		}
	}

	/// Get a configured L4D2 server endpoint by numeric id
	///
	/// # Arguments
	/// * `server_id` - `1` for the primary server, `2` for the secondary server
	///
	/// # Returns
	/// `Some((ip, port))` when the requested server is configured, otherwise `None`
	pub fn server_by_id(&self, server_id: u8) -> Option<(&str, u16)> {
		match server_id {
			1 => self.primary_server(),
			2 => self.secondary_server(),
			_ => None,
		}
	}

	/// Returns dedicated planning chat target when feature is enabled and configured.
	///
	/// # Returns
	/// `Some((chat_group_id, plan_chat_id))` when dedicated planning is enabled and chat id is set,
	/// otherwise `None`.
	pub fn effective_plan_chat(&self) -> Option<(u64, u64)> {
		(self.dedicated_plan_chat && self.plan_chat_id != 0)
			.then_some((self.chat_group_id, self.plan_chat_id))
	}

	/// Returns dedicated poll chat target when feature is enabled and configured.
	///
	/// # Returns
	/// `Some((chat_group_id, poll_chat_id))` when dedicated poll chat is enabled and chat id is set,
	/// otherwise `None`.
	pub fn effective_poll_chat(&self) -> Option<(u64, u64)> {
		(self.dedicated_poll_chat && self.poll_chat_id != 0)
			.then_some((self.chat_group_id, self.poll_chat_id))
	}

	/// Returns dedicated planning chat target for clean-up when feature is enabled and configured.
	///
	/// # Returns
	/// `Some((chat_group_id, plan_chat_id))` when clean-up is enabled and chat id is set,
	/// otherwise `None`.
	pub fn plan_chat_clean_target(&self) -> Option<(u64, u64)> {
		(self.plan_chat_keep_clean && self.plan_chat_id != 0)
			.then_some((self.chat_group_id, self.plan_chat_id))
	}
}

/// Gets the directory containing the current executable
/// 
/// This function determines the directory where the current executable
/// is located. This is used to locate the configuration file and other
/// resources relative to the executable.
/// 
/// # Returns
/// * `Ok(PathBuf)` - The directory containing the executable
/// * `Err(Box<dyn std::error::Error>)` - If the executable path cannot be determined
/// 
/// # Example
/// ```rust
/// match exe_dir() {
///     Ok(dir) => println!("Executable directory: {:?}", dir),
///     Err(e) => eprintln!("Failed to get executable directory: {}", e),
/// }
/// ```
pub fn exe_dir() -> Result<PathBuf, Box<dyn std::error::Error>> {
	match std::env::current_exe() {
		Ok(exe_path) => {
			if let Some(exe_dir) = exe_path.parent() {
				Ok(exe_dir.to_path_buf())
			} else {
				let err = format!("Could not determine the parent directory of the executable.");
				eprintln!("{} {}", "Error:".red(), err);
				return Err(Box::new(QuietErr(Some(err))));
			}
		}
		Err(e) => {
			let err = format!("Failed to get current executable path:\n {}", e);
			eprintln!("{} {}", "Error:".red(), err);
			return Err(Box::new(QuietErr(Some(err))));
		}
	}
}

/// Custom error type for quiet error handling
/// 
/// This struct provides a simple error type that can hold an optional
/// error message. It's used for error handling where detailed error
/// information is not required.
/// 
/// # Fields
/// * `0` - Optional error message string
#[derive(Debug)]
pub struct QuietErr(Option<String>);

impl fmt::Display for QuietErr {
	/// Formats the error for display
	/// 
	/// Returns the error message if present, or an empty string if not.
	/// 
	/// # Arguments
	/// * `f` - The formatter to write to
	/// 
	/// # Returns
	/// * `fmt::Result` - The result of the formatting operation
	fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
		if let Some(ref msg) = self.0 {
			write!(f, "{}", msg)
		} else {
			write!(f, "")
		}
	}
}

impl std::error::Error for QuietErr {}

#[cfg(test)]
mod tests {
	use super::*;

	fn full_toml() -> &'static str {
		r#"
frontend_admins = [76561198000000001, 76561198000000002]

[server]
ip = "127.0.0.1"
port = 27015

[server2]
ip = "127.0.0.2"
port = 27016

[steam]
web_api_key = "key_123"

[steam.bot]
username = "bot_user"
password = "bot_pass"

[steam.chat]
group_id = 1234
chat_id = 5678
commands_without_mention = true
plan_chat_id = 4321
dedicated_plan_chat = true
plan_chat_keep_clean = true
plan_mention_user = true
poll_chat_id = 8765
dedicated_poll_chat = true
poll_chat_remove_command_message = true
poll_mention_user = true

[steam.chat.admin]
admins = [76561198000000003]
admins_same_as_frontend = false
mute_max_minutes = 4320
ignore_muted_commands = false
"#
	}

	#[test]
	fn from_toml_str_parses_expected_fields() {
		let parsed = Config::from_toml_str(full_toml()).expect("expected valid config");
		assert_eq!(parsed.frontend_admins.len(), 2);
		assert_eq!(parsed.steam_web_api_key, "key_123");
		assert_eq!(parsed.chat_group_id, 1234);
		assert_eq!(parsed.chat_id, 5678);
		assert_eq!(parsed.plan_chat_id, 4321);
		assert!(parsed.dedicated_plan_chat);
		assert!(parsed.plan_chat_keep_clean);
		assert!(parsed.plan_mention_user);
		assert_eq!(parsed.poll_chat_id, 8765);
		assert!(parsed.dedicated_poll_chat);
		assert!(parsed.poll_chat_remove_command_message);
		assert!(parsed.poll_mention_user);
		assert_eq!(parsed.steam_account, "bot_user");
		assert_eq!(parsed.steam_password, "bot_pass");
		assert_eq!(parsed.steambot_admins, vec![76561198000000003]);
		assert!(!parsed.steambot_admins_same_as_frontend);
		assert_eq!(parsed.steambot_mute_max_minutes, 4320);
		assert!(!parsed.steambot_ignore_muted_commands);
		assert_eq!(parsed.server_ip, "127.0.0.1");
		assert_eq!(parsed.server_port, 27015);
		assert_eq!(parsed.server2_ip, "127.0.0.2");
		assert_eq!(parsed.server2_port, 27016);
		assert!(parsed.steam_bot_commands_without_mention);
	}

	#[test]
	fn from_toml_str_uses_defaults_for_missing_sections() {
		let parsed = Config::from_toml_str("frontend_admins = []").expect("expected defaults");
		assert_eq!(parsed.frontend_admins, Vec::<i64>::new());
		assert!(parsed.steam_web_api_key.is_empty());
		assert_eq!(parsed.server_port, 0);
		assert_eq!(parsed.server2_port, 0);
		assert_eq!(parsed.chat_group_id, 0);
		assert_eq!(parsed.chat_id, 0);
		assert_eq!(parsed.plan_chat_id, 0);
		assert!(!parsed.dedicated_plan_chat);
		assert!(!parsed.plan_chat_keep_clean);
		assert!(!parsed.plan_mention_user);
		assert_eq!(parsed.poll_chat_id, 0);
		assert!(!parsed.dedicated_poll_chat);
		assert!(!parsed.poll_chat_remove_command_message);
		assert!(!parsed.poll_mention_user);
		assert!(!parsed.steam_bot_commands_without_mention);
		assert!(parsed.steambot_admins.is_empty());
		assert!(parsed.steambot_admins_same_as_frontend);
		assert_eq!(parsed.steambot_mute_max_minutes, 10080);
		assert!(parsed.steambot_ignore_muted_commands);
	}

	#[test]
	fn from_toml_str_rejects_malformed_toml() {
		let result = Config::from_toml_str("this = [");
		assert!(result.is_err());
	}

	#[test]
	fn load_from_reads_and_parses_file() {
		let temp = tempfile::tempdir().expect("tempdir");
		let path = temp.path().join("config.toml");
		std::fs::write(&path, full_toml()).expect("write fixture");

		let loaded = Config::load_from(&path).expect("load from path");
		assert_eq!(loaded.server_ip, "127.0.0.1");
		assert_eq!(loaded.chat_id, 5678);
	}

	#[test]
	fn diff_classifies_live_and_restart_fields() {
		let old = Config::from_toml_str(full_toml()).expect("old");
		let mut new = Config::from_toml_str(full_toml()).expect("new");
		new.server_ip = "10.0.0.1".to_string();
		new.steam_password = "updated".to_string();
		new.plan_mention_user = false;

		let change = old.diff(&new);
		assert!(!change.unchanged);
		assert!(change.live_applied.contains(&"server.ip"));
		assert!(change.live_applied.contains(&"steam.chat.plan_mention_user"));
		assert!(change.requires_restart.contains(&"steam.bot.password"));
	}

	#[test]
	fn effective_plan_chat_requires_enabled_and_chat_id() {
		let mut config = Config::from_toml_str(full_toml()).expect("config");
		assert_eq!(config.effective_plan_chat(), Some((1234, 4321)));

		config.dedicated_plan_chat = false;
		assert_eq!(config.effective_plan_chat(), None);

		config.dedicated_plan_chat = true;
		config.plan_chat_id = 0;
		assert_eq!(config.effective_plan_chat(), None);
	}

	#[test]
	fn plan_chat_clean_target_requires_enabled_and_chat_id() {
		let mut config = Config::from_toml_str(full_toml()).expect("config");
		assert_eq!(config.plan_chat_clean_target(), Some((1234, 4321)));

		config.plan_chat_keep_clean = false;
		assert_eq!(config.plan_chat_clean_target(), None);

		config.plan_chat_keep_clean = true;
		config.plan_chat_id = 0;
		assert_eq!(config.plan_chat_clean_target(), None);
	}

	#[test]
	fn effective_poll_chat_requires_enabled_and_chat_id() {
		let mut config = Config::from_toml_str(full_toml()).expect("config");
		assert_eq!(config.effective_poll_chat(), Some((1234, 8765)));

		config.dedicated_poll_chat = false;
		assert_eq!(config.effective_poll_chat(), None);

		config.dedicated_poll_chat = true;
		config.poll_chat_id = 0;
		assert_eq!(config.effective_poll_chat(), None);
	}

	#[test]
	fn diff_marks_unchanged_when_equal() {
		let old = Config::from_toml_str(full_toml()).expect("old");
		let new = Config::from_toml_str(full_toml()).expect("new");
		let change = old.diff(&new);
		assert!(change.unchanged);
		assert!(change.live_applied.is_empty());
		assert!(change.requires_restart.is_empty());
	}

	#[test]
	fn is_steambot_admin_uses_frontend_when_same_as_frontend() {
		let mut config = Config::from_toml_str(
			r#"
frontend_admins = [76561198000000001]
[steam.chat.admin]
admins_same_as_frontend = true
"#,
		)
		.expect("config");
		assert!(config.is_steambot_admin(76561198000000001));
		assert!(!config.is_steambot_admin(76561198000000099));
		config.steambot_admins = vec![76561198000000099];
		assert!(!config.is_steambot_admin(76561198000000099));
	}

	#[test]
	fn is_steambot_admin_uses_steambot_admins_when_disabled() {
		let config = Config::from_toml_str(
			r#"
frontend_admins = [76561198000000001]
[steam.chat.admin]
admins = [76561198000000099]
admins_same_as_frontend = false
"#,
		)
		.expect("config");
		assert!(!config.is_steambot_admin(76561198000000001));
		assert!(config.is_steambot_admin(76561198000000099));
	}
}
