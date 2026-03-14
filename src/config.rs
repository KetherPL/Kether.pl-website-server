// SPDX-License-Identifier: GPL-3.0-only

use std::{path::PathBuf, fmt};
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
		}
	}
}

impl Default for ServerConfig {
	fn default() -> Self {
		ServerConfig {
			ip: "54.36.179.182".to_string(),
			port: 27015,
		}
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
/// * `steam_account` - Steam account username for bot login
/// * `steam_password` - Steam account password for bot login
/// * `server_ip` - IP address of the L4D2 server
/// * `server_port` - Port number of the L4D2 server
#[derive(Debug)]
pub struct Config {
	pub frontend_admins: Vec<i64>,
	pub steam_web_api_key: String,
	pub chat_group_id: u64,
	pub chat_id: u64,
	pub steam_account: String,
	pub steam_password: String,
	pub server_ip: String,
	pub server_port: u16,
	pub server2_ip: String,
	pub server2_port: u16,
	pub steam_bot_commands_without_mention: bool,
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
		let config_file: ConfigFile = toml::from_str(&content)?;
		
		Ok(Config {
			frontend_admins: config_file.frontend_admins,
			steam_web_api_key: config_file.steam.web_api_key,
			chat_group_id: config_file.steam.chat.group_id,
			chat_id: config_file.steam.chat.chat_id,
			steam_account: config_file.steam.bot.username,
			steam_password: config_file.steam.bot.password,
			server_ip: config_file.server.ip,
			server_port: config_file.server.port,
			server2_ip: config_file.server2.ip,
			server2_port: config_file.server2.port,
			steam_bot_commands_without_mention: config_file.steam.chat.commands_without_mention,
		})
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
