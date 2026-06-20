// SPDX-License-Identifier: GPL-3.0-only

use crate::config::{CONF_FILE_NAME, Config, ConfigChange, exe_dir};
use crate::steam_bot::bot::SteamBot;
use once_cell::sync::OnceCell;
use std::sync::{Arc, RwLock};

/// Global registry for SteamBot instance and configuration
/// 
/// This module provides thread-safe access to the global SteamBot instance
/// and configuration using OnceCell for lazy initialization. This allows
/// other parts of the application to access the SteamBot without needing
/// direct references.
/// 
/// # Thread Safety
/// All functions in this module are thread-safe. OnceCell ensures that
/// initialization happens only once, even in concurrent scenarios.

static STEAM_BOT: OnceCell<Arc<SteamBot>> = OnceCell::new();
static CONFIG: OnceCell<ConfigHandle> = OnceCell::new();

pub type ConfigHandle = Arc<RwLock<Arc<Config>>>;

/// Sets the global SteamBot instance
/// 
/// This function should be called once during initialization to register
/// the SteamBot instance for global access.
/// 
/// # Arguments
/// * `bot` - The SteamBot instance to register
/// 
/// # Panics
/// Panics if called more than once, as the global instance can only be set once.
pub fn set_bot(bot: Arc<SteamBot>) {
    STEAM_BOT.set(bot).expect("SteamBot already initialized");
}

/// Gets the global SteamBot instance
/// 
/// # Returns
/// `Some(&Arc<SteamBot>)` if the instance has been initialized, `None` otherwise
pub fn bot() -> Option<&'static Arc<SteamBot>> {
    STEAM_BOT.get()
}

/// Sets the global configuration
/// 
/// This function should be called once during initialization to register
/// the configuration for global access.
/// 
/// # Arguments
/// * `config` - The configuration to register
/// 
/// # Panics
/// Panics if called more than once, as the global config can only be set once.
pub fn set_config(config: Config) {
    if let Some(handle) = CONFIG.get() {
        if let Ok(mut guard) = handle.write() {
            *guard = Arc::new(config);
            return;
        }
        panic!("Config lock poisoned");
    }

    CONFIG
        .set(Arc::new(RwLock::new(Arc::new(config))))
        .expect("Config already initialized");
}

/// Sets the global configuration if it hasn't been set yet
/// 
/// This is a non-panicking version of `set_config()` that allows
/// setting the config only if it hasn't been initialized yet.
/// 
/// # Arguments
/// * `config` - The configuration to register (only if not already set)
pub fn set_config_if_absent(config: Config) {
    let _ = CONFIG.set(Arc::new(RwLock::new(Arc::new(config))));
}

/// Gets the global configuration
/// 
/// # Returns
/// A reference to the global configuration
/// 
/// # Panics
/// Panics if the configuration has not been initialized. Use `set_config()`
/// or `set_config_if_absent()` before calling this function.
pub fn config() -> Arc<Config> {
    let handle = CONFIG.get().expect("Config not initialized");
    match handle.read() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            eprintln!("Warning: Config lock poisoned: {}", e);
            e.into_inner().clone()
        }
    }
}

/// Gets the global configuration if available
/// 
/// # Returns
/// `Some(&Config)` if the configuration has been initialized, `None` otherwise
pub fn config_opt() -> Option<Arc<Config>> {
    CONFIG.get().map(|handle| match handle.read() {
        Ok(guard) => guard.clone(),
        Err(e) => {
            eprintln!("Warning: Config lock poisoned: {}", e);
            e.into_inner().clone()
        }
    })
}

pub fn config_handle() -> ConfigHandle {
    if let Some(handle) = CONFIG.get() {
        return handle.clone();
    }

    let config_path = exe_dir()
        .map(|d| d.join(CONF_FILE_NAME))
        .expect("Failed to get executable directory for config handle");
    let loaded = Config::load_from(&config_path)
        .expect("Failed to load configuration for shared config handle");
    let handle = Arc::new(RwLock::new(Arc::new(loaded)));
    let _ = CONFIG.set(handle.clone());
    handle
}

pub fn reload_config(new_config: Config) -> ConfigChange {
    let handle = config_handle();
    match handle.write() {
        Ok(mut guard) => {
            let previous = guard.clone();
            let change = previous.diff(&new_config);
            *guard = Arc::new(new_config);
            change
        }
        Err(e) => {
            eprintln!("Warning: Config lock poisoned during reload: {}", e);
            let mut guard = e.into_inner();
            let previous = guard.clone();
            let change = previous.diff(&new_config);
            *guard = Arc::new(new_config);
            change
        }
    }
}

