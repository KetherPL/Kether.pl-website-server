use diesel::{r2d2::{ConnectionManager, Pool}, SqliteConnection};
use std::{path::PathBuf, string::ParseError};

use crate::config::{exe_dir, Config, DATABASE_PATH, DATABASE_RELATIVE_DIR};

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

fn db_path() -> Result<PathBuf, ParseError> {
    let config = match Config::load() {
        Ok(cfg) => cfg,
        Err(e) => {
            eprintln!("Failed to load configuration: {}. Using hardcoded defaults.", e);
            // Use hardcoded values from config.rs
            Config {
                database_path: DATABASE_PATH.to_string(),
                database_relative_dir: DATABASE_RELATIVE_DIR,
                steam_web_api_key: "".to_string(),               
            }
        }
    };

    let db_path = if config.database_relative_dir {
        exe_dir()
            .expect("Failed to get executable's directory")
            .join(&config.database_path)
    } else {
        PathBuf::from(&config.database_path)
    };

    if !db_path.exists() {
        eprintln!("{} doesn't exist. Maybe create it with 'diesel setup', or obtain a created one?", db_path.display());
        std::process::exit(1);
    }
    Ok(db_path)
}

pub fn establish_connection_pool() -> DbPool {
    let database_path = db_path().expect("Failed to get database path");
    // Convert PathBuf to String, because ConnectionManager won't accept PathBuf
    let database_url = database_path.to_str().expect("Failed to convert path to string").to_string();
    let manager = ConnectionManager::<SqliteConnection>::new(database_url);
    Pool::builder()
        .build(manager)
        .expect("Failed to create pool.")
}
