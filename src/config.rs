// SPDX-License-Identifier: GPL-3.0-only

use ini::Ini;
use std::{path::PathBuf, fmt};
use colored::Colorize;

/// Configuration file name constant
/// 
/// The name of the configuration file that must be in the same directory
/// as the executable or in its subdirectory.
pub const CONF_FILE_NAME: &str = "KISS.ini";

/// Default database path constant
/// 
/// Hardcoded database path in case the DB_PATH environment variable
/// would be unavailable. Used as fallback for database configuration.
pub const DATABASE_PATH: &str = "kether.sqlite";

/// Database relative directory setting
/// 
/// Determines if the database is in the same directory as the executable.
/// If true, just put a file NAME in the DATABASE_PATH.
/// If false, use any directory that the shell is already in.
pub const DATABASE_RELATIVE_DIR: bool = true;

/// Configuration structure for the Kether Internal Services Server
/// 
/// This struct holds all configuration values loaded from the KISS.ini file.
/// It provides a centralized way to access all application settings.
/// 
/// # Fields
/// * `steam_web_api_key` - Steam Web API key for fetching user data
/// * `chat_group_id` - Steam group ID for chat functionality
/// * `chat_id` - Steam chat ID for message sending
/// * `steam_account` - Steam account username for bot login
/// * `steam_password` - Steam account password for bot login
/// 
/// # Example
/// ```rust
/// let config = Config::load()?;
/// println!("Steam API key: {}", config.steam_web_api_key);
/// ```
#[derive(Debug)]
pub struct Config {
	pub steam_web_api_key: String,
	pub chat_group_id: u64,
	pub chat_id: u64,
	pub steam_account: String,
	pub steam_password: String,
}

impl Config {
	/// Loads configuration from the KISS.ini file
	/// 
	/// This function reads the configuration file and creates a Config struct
	/// with all the necessary settings. If the config file doesn't exist,
	/// it creates a new one with default values.
	/// 
	/// The function performs the following operations:
	/// 1. Locates the KISS.ini file in the executable directory
	/// 2. Loads existing configuration or creates a new one
	/// 3. Validates and adds missing configuration keys
	/// 4. Parses all values into the appropriate types
	/// 5. Returns a Config struct with all settings
	/// 
	/// # Returns
	/// * `Ok(Config)` - Successfully loaded configuration
	/// * `Err(Box<dyn std::error::Error>)` - If loading or parsing fails
	/// 
	/// # Example
	/// ```rust
	/// match Config::load() {
	///     Ok(config) => {
	///         println!("Configuration loaded successfully");
	///         // Use config.database_path, config.steam_web_api_key, etc.
	///     },
	///     Err(e) => eprintln!("Failed to load config: {}", e),
	/// }
	/// ```
	pub fn load() -> Result<Self, Box<dyn std::error::Error>> {
		let conf_path = exe_dir()?.join(CONF_FILE_NAME);

		let mut ini = match Ini::load_from_file(&conf_path) {
			Ok(loaded_ini) => {
				println!("Config file loaded successfully.");
				loaded_ini
			}
			Err(e) => {
				eprintln!("Error loading config file: {}", e);
				println!("Creating new config file at: {}", conf_path.display());
				let mut newconf = Ini::new();
				newconf.with_general_section()
					.set(";; The database.sqlite file path or file name if the database_relative_dir option is true. ", "")
					.set("database_path", DATABASE_PATH)
					.set(";; Is the database in the same dir as the executable? If yes, don't forget to put just a file NAME in the database_path option. ", "")
					.set(";; False ", " any directory that the shell is already in")
					.set("database_relative_dir", DATABASE_RELATIVE_DIR.to_string())
					.set(";; Steam Web API key that will be utilized to fetch Steam user data (e.g. name, avatar, etc).", "")
					.set("steam_web_api_key", "")
					.set(";; Group ID and Chat ID that will be utilized to access the selected Steam Group Chat (for posting !sub)", "")
					.set("chat_group_id", "")
					.set("chat_id", "")
					.set(";; Steam account login data for the bot instance (for posting !sub)", "")
					.set("steam_account", "")
					.set("steam_password", "");
				newconf.write_to_file(&conf_path)?;
				newconf
			}
		};

		let mut changed = false;
		{
			let sec = ini.general_section_mut();

			if !sec.contains_key("database_path") {
				eprintln!("Key 'database_path' not found in {}. Adding with default value: {}", CONF_FILE_NAME, DATABASE_PATH);
				sec.insert("database_path", DATABASE_PATH);
				changed = true;
			}

			if !sec.contains_key("database_relative_dir") {
				eprintln!("Key 'database_relative_dir' not found in {}. Adding with default value: {}", CONF_FILE_NAME, DATABASE_RELATIVE_DIR);
				sec.insert("database_relative_dir", DATABASE_RELATIVE_DIR.to_string());
				changed = true;
			}

			if !sec.contains_key("steam_web_api_key") {
				eprintln!("Key 'steam_web_api_key' not found in {}. Adding...", CONF_FILE_NAME);
				sec.insert("steam_web_api_key", "");
				changed = true;
			}

			if !sec.contains_key("chat_group_id") {
				eprintln!("Key 'chat_group_id' not found in {}. Adding...", CONF_FILE_NAME);
				sec.insert("chat_group_id", "");
				changed = true;
			}

			if !sec.contains_key("chat_id") {
				eprintln!("Key 'chat_id' not found in {}. Adding...", CONF_FILE_NAME);
				sec.insert("chat_id", "");
				changed = true;
			}

			if !sec.contains_key("steam_account") {
				eprintln!("Key 'steam_account' not found in {}. Adding...", CONF_FILE_NAME);
				sec.insert("steam_account", "");
				changed = true;
			}

			if !sec.contains_key("steam_password") {
				eprintln!("Key 'steam_password' not found in {}. Adding...", CONF_FILE_NAME);
				sec.insert("steam_password", "");
				changed = true;
			}
		} // sec goes out of scope here, releasing the mutable borrow

		if changed {
			eprintln!("Saving updated config file to: {}", conf_path.display());
			ini.write_to_file(&conf_path)?;
		}

	// Now it's safe to immutably borrow `ini`
	let sec = ini.general_section();
	let steam_web_api_key = sec.get("steam_web_api_key").unwrap().to_string();
	let chat_group_id = sec.get("chat_group_id").unwrap().parse::<u64>().unwrap_or(0);
	let chat_id = sec.get("chat_id").unwrap().parse::<u64>().unwrap_or(0);
	let steam_account = sec.get("steam_account").unwrap().to_string();
	let steam_password = sec.get("steam_password").unwrap().to_string();
	

	Ok(Config {
		steam_web_api_key,
		chat_group_id,
		chat_id,
		steam_account,
		steam_password,
	})
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
