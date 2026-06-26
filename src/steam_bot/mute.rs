// SPDX-License-Identifier: GPL-3.0-only

use once_cell::sync::OnceCell;
use rocket::serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::RwLock;
use std::time::{SystemTime, UNIX_EPOCH};

const MEMORY_ONLY_MAX_MINUTES: u64 = 720;
const SWEEP_INTERVAL_SECS: u64 = 30;
const MUTED_USERS_FILE: &str = "muted_users.json";

#[derive(Debug, Clone)]
struct MuteEntry {
    unmute_at: i64,
    persisted: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(crate = "rocket::serde")]
struct MutedUserRecord {
    steam_id: u64,
    unmute_at: i64,
}

static MUTES: OnceCell<RwLock<HashMap<u64, MuteEntry>>> = OnceCell::new();
static SWEEPER_STARTED: AtomicBool = AtomicBool::new(false);

fn mutes() -> &'static RwLock<HashMap<u64, MuteEntry>> {
    MUTES.get_or_init(|| RwLock::new(HashMap::new()))
}

fn now_unix() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

fn muted_users_path() -> Result<std::path::PathBuf, String> {
    crate::config::exe_dir()
        .map(|dir| dir.join(MUTED_USERS_FILE))
        .map_err(|e| format!("Failed to get executable directory: {}", e))
}

async fn load_from_disk() -> Result<Vec<MutedUserRecord>, String> {
    let path = muted_users_path()?;
    let content = match smol::fs::read_to_string(&path).await {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(format!("Failed to read {}: {}", path.display(), e)),
    };

    rocket::serde::json::from_str(&content)
        .map_err(|e| format!("Failed to parse {}: {}", path.display(), e))
}

async fn save_to_disk(records: &Vec<MutedUserRecord>) -> Result<(), String> {
    let path = muted_users_path()?;
    let json = rocket::serde::json::to_pretty_string(records)
        .map_err(|e| format!("Failed to serialize muted users: {}", e))?;
    let temp_path = path.with_extension("tmp");
    smol::fs::write(&temp_path, json)
        .await
        .map_err(|e| format!("Failed to write temp file: {}", e))?;
    smol::fs::rename(&temp_path, &path)
        .await
        .map_err(|e| format!("Failed to rename temp file: {}", e))?;
    Ok(())
}

fn collect_persisted_records(map: &HashMap<u64, MuteEntry>) -> Vec<MutedUserRecord> {
    let now = now_unix();
    let mut records: Vec<MutedUserRecord> = map
        .iter()
        .filter(|(_, entry)| entry.persisted && entry.unmute_at > now)
        .map(|(steam_id, entry)| MutedUserRecord {
            steam_id: *steam_id,
            unmute_at: entry.unmute_at,
        })
        .collect();
    records.sort_by_key(|record| record.steam_id);
    records
}

async fn persist() -> Result<(), String> {
    let records = {
        let guard = mutes()
            .read()
            .map_err(|e| format!("Mute store read lock poisoned: {}", e))?;
        collect_persisted_records(&guard)
    };
    save_to_disk(&records).await
}

fn purge_expired(map: &mut HashMap<u64, MuteEntry>) -> bool {
    let now = now_unix();
    let mut any_persisted_removed = false;
    map.retain(|_, entry| {
        if entry.unmute_at <= now {
            if entry.persisted {
                any_persisted_removed = true;
            }
            false
        } else {
            true
        }
    });
    any_persisted_removed
}

/// Loads persisted mutes and starts the background expiry sweeper.
pub async fn init() {
    let now = now_unix();
    if let Ok(records) = load_from_disk().await {
        if let Ok(mut guard) = mutes().write() {
            for record in records {
                if record.unmute_at <= now {
                    continue;
                }
                guard.insert(
                    record.steam_id,
                    MuteEntry {
                        unmute_at: record.unmute_at,
                        persisted: true,
                    },
                );
            }
        }
    }

    if SWEEPER_STARTED
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
    {
        tokio::spawn(async {
            let mut interval = tokio::time::interval(tokio::time::Duration::from_secs(SWEEP_INTERVAL_SECS));
            interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
            loop {
                interval.tick().await;
                let needs_persist = {
                    match mutes().write() {
                        Ok(mut guard) => purge_expired(&mut guard),
                        Err(e) => {
                            eprintln!("Mute sweeper: lock poisoned: {}", e);
                            false
                        }
                    }
                };
                if needs_persist
                    && let Err(e) = persist().await
                {
                    eprintln!("Mute sweeper: failed to persist muted users: {}", e);
                }
            }
        });
    }
}

/// Adds or extends a mute for the given Steam ID. Returns the unmute timestamp.
pub async fn add_mute(steam_id: u64, duration_minutes: u64) -> i64 {
    let unmute_at = now_unix() + (duration_minutes as i64) * 60;
    let persisted = duration_minutes > MEMORY_ONLY_MAX_MINUTES;
    let should_persist = {
        let mut guard = mutes().write().expect("Mute store write lock poisoned");
        guard.insert(
            steam_id,
            MuteEntry {
                unmute_at,
                persisted,
            },
        );
        persisted
    };

    if should_persist
        && let Err(e) = persist().await
    {
        eprintln!("Failed to persist mute for {}: {}", steam_id, e);
    }

    unmute_at
}

/// Removes a mute. Returns whether the user was muted.
pub async fn remove_mute(steam_id: u64) -> bool {
    let removed = {
        let mut guard = mutes().write().expect("Mute store write lock poisoned");
        guard.remove(&steam_id)
    };

    let Some(entry) = removed else {
        return false;
    };

    if entry.persisted
        && let Err(e) = persist().await
    {
        eprintln!("Failed to persist unmute for {}: {}", steam_id, e);
    }

    true
}

/// Returns whether the Steam ID is currently muted.
pub fn is_muted(steam_id: u64) -> bool {
    let Ok(guard) = mutes().read() else {
        return false;
    };
    guard
        .get(&steam_id)
        .is_some_and(|entry| entry.unmute_at > now_unix())
}

/// Returns active mutes sorted by unmute time.
pub fn list_mutes() -> Vec<(u64, i64)> {
    let Ok(guard) = mutes().read() else {
        return Vec::new();
    };
    let now = now_unix();
    let mut entries: Vec<(u64, i64)> = guard
        .iter()
        .filter(|(_, entry)| entry.unmute_at > now)
        .map(|(steam_id, entry)| (*steam_id, entry.unmute_at))
        .collect();
    entries.sort_by_key(|(_, unmute_at)| *unmute_at);
    entries
}
