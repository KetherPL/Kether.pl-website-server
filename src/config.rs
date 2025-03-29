// SPDX-License-Identifier: GPL-3.0-only

use ini::Ini;
use std::{path::PathBuf, fmt};
use colored::Colorize;

pub const CONF_FILE_NAME: &str = "KISS.ini"; // Config name (must be in the same dir as executable, or in it's subdir)
pub const DATABASE_PATH: &str = "kether.sqlite"; // Hardcoded in case if DB_PATH env var would be unavailable
pub const DATABASE_RELATIVE_DIR: bool = true; /* Is the database in the same dir as the executable?
                                        If yes, just put a file NAME in the DATABASE_PATH */

#[derive(Debug)]
pub struct Config {
    pub database_path: String,
    pub database_relative_dir: bool,
}

impl Config {
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
                    .set(";; Is the database in the same dir as the executable? If yes, don't forget to put just a file NAME in the database_path option.", "")
                    .set(";; False ", " any directory that the shell is already in")
                    .set("database_relative_dir", DATABASE_RELATIVE_DIR.to_string());
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
        } // sec goes out of scope here, releasing the mutable borrow

        if changed {
            eprintln!("Saving updated config file to: {}", conf_path.display());
            ini.write_to_file(&conf_path)?;
        }

        // Now it's safe to immutably borrow `ini`
        let sec = ini.general_section();
        let db_path = sec.get("database_path").unwrap().to_string();
        let db_relative_dir = sec.get("database_relative_dir").unwrap().parse::<bool>().unwrap();

        Ok(Config {
            database_path: db_path,
            database_relative_dir: db_relative_dir,
        })
    }
}

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

#[derive(Debug)]
pub struct QuietErr(Option<String>);
impl fmt::Display for QuietErr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ref msg) = self.0 {
            write!(f, "{}", msg)
        } else {
            write!(f, "")
        }
    }
}
impl std::error::Error for QuietErr {}
