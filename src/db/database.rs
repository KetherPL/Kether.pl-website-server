use diesel::{r2d2::{ConnectionManager, Pool}, SqliteConnection};
use dotenv::dotenv;
use std::{env, path::PathBuf, string::ParseError, env::VarError};

use crate::DATABASE_PATH;

pub type DbPool = Pool<ConnectionManager<SqliteConnection>>;

fn db_path() -> Result<PathBuf, ParseError> {
    dotenv().ok();

    match env::var("DB_PATH") {
        Ok(path) => {
            let db_path: PathBuf = path.into();
            if !db_path.exists() {
                eprintln!("{} doesn't exist. Maybe create it with 'diesel setup', or obtain a created one?", db_path.display())
            }
            Ok(db_path)
        }
        Err(VarError::NotPresent) => {
            Ok(DATABASE_PATH.into())
        }
        Err(e) => {
            eprintln!("Error reading DB_PATH environment variable: {}", e);
            Ok(DATABASE_PATH.into())
        }
    }
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
