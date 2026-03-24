// SPDX-License-Identifier: GPL-3.0-only

use tokio::sync::{broadcast, mpsc};
use once_cell::sync::OnceCell;
use std::collections::HashMap;
use std::net::IpAddr;
use std::sync::{
    atomic::{AtomicU64, Ordering},
    Arc, Mutex,
};

/// Global broadcast channel for plan timestamps and CLEAR messages
/// 
/// This channel allows broadcasting "SET <timestamp>" or "CLEAR" messages from the `!plan` command
/// to all connected WebSocket clients.
static PLAN_BROADCASTER: OnceCell<broadcast::Sender<String>> = OnceCell::new();

/// Stores the last set reservation timestamp
static LAST_TIMESTAMP: OnceCell<Arc<Mutex<Option<i64>>>> = OnceCell::new();

/// Stores the handle for the expiration checker task
static EXPIRATION_TASK: OnceCell<Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>> = OnceCell::new();

type TargetedConnections = HashMap<IpAddr, HashMap<u64, mpsc::UnboundedSender<String>>>;
type TargetedTimestamps = HashMap<IpAddr, i64>;
type TargetedTasks = HashMap<IpAddr, tokio::task::JoinHandle<()>>;

/// Stores targeted WebSocket connections keyed by direct peer IP.
static TARGETED_CONNECTIONS: OnceCell<Arc<Mutex<TargetedConnections>>> = OnceCell::new();

/// Stores the last targeted reservation timestamp per IP.
static TARGETED_TIMESTAMPS: OnceCell<Arc<Mutex<TargetedTimestamps>>> = OnceCell::new();

/// Stores expiration tasks for targeted reservations keyed by IP.
static TARGETED_EXPIRATION_TASKS: OnceCell<Arc<Mutex<TargetedTasks>>> = OnceCell::new();

/// Generates unique ids for registered WebSocket connections.
static NEXT_CONNECTION_ID: OnceCell<AtomicU64> = OnceCell::new();

/// Max sleep between expiration polls (seconds); also caps per-iteration wait when expiry is soon.
#[cfg(feature = "rest_api")]
const EXPIRATION_POLL_CAP_SECS: i64 = 60;

/// Initializes the broadcast channel for plan timestamps
/// 
/// This function should be called once during application startup (typically
/// in the Rocket initialization). It creates a broadcast channel with a
/// buffer capacity of 100 messages and initializes state tracking.
/// 
/// # Returns
/// A receiver that can be used for testing or debugging purposes
/// 
/// # Panics
/// Panics if called more than once, as the broadcaster can only be initialized once.
pub fn init_broadcaster() -> broadcast::Receiver<String> {
    let (tx, rx) = broadcast::channel(100);
    PLAN_BROADCASTER.set(tx).expect("Broadcaster already initialized");
    
    // Initialize state tracking
    LAST_TIMESTAMP.set(Arc::new(Mutex::new(None))).expect("Already initialized");
    EXPIRATION_TASK.set(Arc::new(Mutex::new(None))).expect("Already initialized");
    TARGETED_CONNECTIONS
        .set(Arc::new(Mutex::new(HashMap::new())))
        .expect("Already initialized");
    TARGETED_TIMESTAMPS
        .set(Arc::new(Mutex::new(HashMap::new())))
        .expect("Already initialized");
    TARGETED_EXPIRATION_TASKS
        .set(Arc::new(Mutex::new(HashMap::new())))
        .expect("Already initialized");
    NEXT_CONNECTION_ID
        .set(AtomicU64::new(1))
        .expect("Already initialized");
    
    rx
}

/// Sets a reservation timestamp and starts expiration checking
/// 
/// This function stores the timestamp, cancels any existing expiration task,
/// starts a new expiration checker, and broadcasts "SET <timestamp>" to all
/// WebSocket clients.
/// 
/// # Arguments
/// * `timestamp` - The Unix timestamp to set (as i64)
/// 
/// # Note
/// This function gracefully handles the case where the broadcaster hasn't been
/// initialized yet (e.g., if called before Rocket starts). In such cases, it
/// does nothing.
#[cfg(feature = "rest_api")]
pub fn set_reservation_timestamp(timestamp: i64) {
    clear_all_targeted_reservations();

    // Store the timestamp
    if let Some(last_ts) = LAST_TIMESTAMP.get() {
        let mut ts_guard = match last_ts.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when setting timestamp: {}", e);
                let mut guard = e.into_inner();
                *guard = Some(timestamp);
                guard
            }
        };
        *ts_guard = Some(timestamp);
    }
    
    // Cancel any existing expiration task
    if let Some(task_handle) = EXPIRATION_TASK.get() {
        let mut handle_guard = match task_handle.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when canceling expiration task: {}", e);
                let mut guard = e.into_inner();
                if let Some(handle) = guard.take() {
                    handle.abort();
                }
                guard
            }
        };
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }
    }
    
    // Start new expiration checker
    start_expiration_checker(timestamp);
    
    // Broadcast "SET <timestamp>" message
    if let Some(tx) = PLAN_BROADCASTER.get() {
        let _ = tx.send(format!("SET {}", timestamp)); // Ignore errors if no receivers
    }
}

/// Clears the reservation and broadcasts "CLEAR" message
/// 
/// This function clears **both** the global plan and **all** per-IP targeted plans,
/// cancels expiration tasks, notifies targeted WebSocket clients, and broadcasts
/// `"CLEAR"` on the global channel (so non-targeted subscribers also reset).
/// 
/// # Note
/// This function gracefully handles the case where the broadcaster hasn't been
/// initialized yet (e.g., if called before Rocket starts). In such cases, it
/// does nothing.
#[cfg(feature = "rest_api")]
pub fn clear_reservation() {
    clear_all_targeted_reservations();
    clear_global_state();

    // Broadcast "CLEAR" message
    if let Some(tx) = PLAN_BROADCASTER.get() {
        let _ = tx.send("CLEAR".to_string()); // Ignore errors if no receivers
    }
}

#[cfg(feature = "rest_api")]
fn current_global_message() -> String {
    match get_current_timestamp() {
        Some(timestamp) if chrono::Utc::now().timestamp() < timestamp => format!("SET {}", timestamp),
        _ => "CLEAR".to_string(),
    }
}

#[cfg(feature = "rest_api")]
fn parse_target_ip(ip: &str) -> Option<IpAddr> {
    ip.parse().ok()
}

#[cfg(feature = "rest_api")]
fn clear_global_state() {
    if let Some(last_ts) = LAST_TIMESTAMP.get() {
        let mut ts_guard = match last_ts.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when clearing timestamp: {}", e);
                let mut guard = e.into_inner();
                *guard = None;
                guard
            }
        };
        *ts_guard = None;
    }

    if let Some(task_handle) = EXPIRATION_TASK.get() {
        let mut handle_guard = match task_handle.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when canceling expiration task: {}", e);
                let mut guard = e.into_inner();
                if let Some(handle) = guard.take() {
                    handle.abort();
                }
                guard
            }
        };
        if let Some(handle) = handle_guard.take() {
            handle.abort();
        }
    }
}

#[cfg(feature = "rest_api")]
fn send_targeted_message(target_ip: IpAddr, message: String) {
    let Some(connections) = TARGETED_CONNECTIONS.get() else {
        return;
    };

    let mut stale_connection_ids = Vec::new();
    let mut guard = match connections.lock() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Warning: Mutex poisoned when sending targeted message: {}", e);
            e.into_inner()
        }
    };

    if let Some(senders) = guard.get_mut(&target_ip) {
        for (&connection_id, sender) in senders.iter() {
            if sender.send(message.clone()).is_err() {
                stale_connection_ids.push(connection_id);
            }
        }

        for connection_id in stale_connection_ids {
            senders.remove(&connection_id);
        }

        if senders.is_empty() {
            guard.remove(&target_ip);
        }
    }
}

/// Starts a background task that checks if the reservation timestamp has expired
/// 
/// This function spawns a task that checks immediately, then every 60 seconds if the current time
/// has passed the reservation timestamp. When expired, it broadcasts "CLEAR" and
/// clears the stored timestamp.
/// 
/// # Arguments
/// * `timestamp` - The Unix timestamp to check against
#[cfg(feature = "rest_api")]
fn start_expiration_checker(timestamp: i64) {
    let last_ts = LAST_TIMESTAMP.get().expect("LAST_TIMESTAMP not initialized").clone();
    let task_handle = EXPIRATION_TASK.get().expect("EXPIRATION_TASK not initialized").clone();
    let task_handle_for_closure = task_handle.clone();
    
    // Try to get the current runtime handle
    let rt_handle = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle,
        Err(e) => {
            // If we're not in a tokio runtime, we can't spawn tasks
            // This should not happen in normal operation, but we handle it gracefully
            eprintln!("Error: Cannot spawn expiration checker task - not in tokio runtime: {}", e);
            eprintln!("This usually means set_reservation_timestamp() was called from outside an async context");
            return;
        }
    };
    
    let handle = rt_handle.spawn(async move {
        println!("Expiration checker started for timestamp: {} (current time: {})", timestamp, chrono::Utc::now().timestamp());
        
        loop {
            // Check if timestamp has passed
            let current_time = chrono::Utc::now().timestamp();
            if current_time >= timestamp {
                // Timestamp has expired, clear reservation
                println!("Timestamp {} has expired (current: {}), clearing reservation", timestamp, current_time);
                
                let mut ts_guard = match last_ts.lock() {
                    Ok(guard) => guard,
                    Err(e) => {
                        eprintln!("Warning: Mutex poisoned in expiration checker: {}", e);
                        let mut guard = e.into_inner();
                        // Still try to clear if it matches
                        if guard.as_ref().map(|&ts| ts == timestamp).unwrap_or(false) {
                            *guard = None;
                            if let Some(tx) = PLAN_BROADCASTER.get() {
                                let _ = tx.send("CLEAR".to_string());
                                println!("Broadcasted CLEAR message");
                            }
                        }
                        break;
                    }
                };
                
                // Only clear if this is still the active timestamp
                if ts_guard.as_ref().map(|&ts| ts == timestamp).unwrap_or(false) {
                    *ts_guard = None;
                    println!("Cleared reservation timestamp");
                    
                    // Broadcast "CLEAR"
                    if let Some(tx) = PLAN_BROADCASTER.get() {
                        let _ = tx.send("CLEAR".to_string());
                        println!("Broadcasted CLEAR message");
                    }
                } else {
                    println!("Timestamp changed (current stored: {:?}), not clearing", *ts_guard);
                }
                
                // Cancel this task
                let mut handle_guard = match task_handle_for_closure.lock() {
                    Ok(guard) => guard,
                    Err(e) => {
                        eprintln!("Warning: Mutex poisoned when cleaning up task handle: {}", e);
                        e.into_inner().take();
                        break;
                    }
                };
                handle_guard.take();
                break;
            }
            
            // Calculate how long to wait: either until timestamp expires, or cap, whichever is shorter
            let time_until_expiry = timestamp - current_time;
            let wait_duration = if time_until_expiry > 0 && time_until_expiry < EXPIRATION_POLL_CAP_SECS {
                time_until_expiry
            } else {
                EXPIRATION_POLL_CAP_SECS
            };
            
            println!("Expiration checker: waiting {} seconds (timestamp expires in {} seconds)", wait_duration, time_until_expiry);
            tokio::time::sleep(tokio::time::Duration::from_secs(wait_duration as u64)).await;
        }
        
        println!("Expiration checker task completed");
    });
    
    // Store the task handle
    let mut handle_guard = match task_handle.lock() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Warning: Mutex poisoned when storing task handle: {}", e);
            let mut guard = e.into_inner();
            *guard = Some(handle);
            return;
        }
    };
    *handle_guard = Some(handle);
    println!("Expiration checker task spawned and stored");
}

#[cfg(feature = "rest_api")]
fn start_targeted_expiration_checker(target_ip: IpAddr, timestamp: i64) {
    let timestamps = TARGETED_TIMESTAMPS
        .get()
        .expect("TARGETED_TIMESTAMPS not initialized")
        .clone();
    let tasks = TARGETED_EXPIRATION_TASKS
        .get()
        .expect("TARGETED_EXPIRATION_TASKS not initialized")
        .clone();
    let tasks_for_closure = tasks.clone();

    let rt_handle = match tokio::runtime::Handle::try_current() {
        Ok(handle) => handle,
        Err(e) => {
            eprintln!("Error: Cannot spawn targeted expiration checker task - not in tokio runtime: {}", e);
            return;
        }
    };

    let handle = rt_handle.spawn(async move {
        loop {
            let current_time = chrono::Utc::now().timestamp();
            if current_time >= timestamp {
                let mut should_clear = false;
                {
                    let mut guard = match timestamps.lock() {
                        Ok(guard) => guard,
                        Err(e) => {
                            eprintln!("Warning: Mutex poisoned in targeted expiration checker: {}", e);
                            e.into_inner()
                        }
                    };

                    if guard.get(&target_ip).copied() == Some(timestamp) {
                        guard.remove(&target_ip);
                        should_clear = true;
                    }
                }

                {
                    let mut guard = match tasks_for_closure.lock() {
                        Ok(guard) => guard,
                        Err(e) => {
                            eprintln!("Warning: Mutex poisoned when cleaning targeted expiration task: {}", e);
                            e.into_inner()
                        }
                    };
                    guard.remove(&target_ip);
                }

                if should_clear {
                    send_targeted_message(target_ip, current_global_message());
                }

                break;
            }

            let time_until_expiry = timestamp - current_time;
            let wait_duration = if time_until_expiry > 0 && time_until_expiry < EXPIRATION_POLL_CAP_SECS {
                time_until_expiry
            } else {
                EXPIRATION_POLL_CAP_SECS
            };

            tokio::time::sleep(tokio::time::Duration::from_secs(wait_duration as u64)).await;
        }
    });

    let mut guard = match tasks.lock() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Warning: Mutex poisoned when storing targeted task handle: {}", e);
            e.into_inner()
        }
    };
    if let Some(previous_handle) = guard.insert(target_ip, handle) {
        previous_handle.abort();
    }
}


/// Gets the current reservation timestamp, if any
/// 
/// This function returns the currently stored reservation timestamp.
/// It's useful for sending the current state to newly connected WebSocket clients.
/// 
/// # Returns
/// * `Some(timestamp)` - If there's an active reservation
/// * `None` - If no reservation is currently set
/// 
/// # Note
/// This function gracefully handles the case where the state hasn't been
/// initialized yet (e.g., if called before Rocket starts). In such cases, it
/// returns `None`.
#[cfg(feature = "rest_api")]
pub fn get_current_timestamp() -> Option<i64> {
    if let Some(last_ts) = LAST_TIMESTAMP.get() {
        let ts_guard = match last_ts.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when getting current timestamp: {}", e);
                return None;
            }
        };
        *ts_guard
    } else {
        None
    }
}

/// Gets the current targeted reservation timestamp for a configured IP string.
#[cfg(feature = "rest_api")]
pub fn get_targeted_timestamp(ip: &str) -> Option<i64> {
    let parsed_ip = parse_target_ip(ip)?;
    get_targeted_timestamp_for_ip(parsed_ip)
}

/// Gets the current targeted reservation timestamp for a direct peer IP.
#[cfg(feature = "rest_api")]
pub fn get_targeted_timestamp_for_ip(target_ip: IpAddr) -> Option<i64> {
    has_targeted_timestamp(target_ip).then(|| {
        TARGETED_TIMESTAMPS
            .get()
            .and_then(|timestamps| timestamps.lock().ok())
            .and_then(|guard| guard.get(&target_ip).copied())
    }).flatten()
}

/// Returns whether an IP currently has an active targeted reservation.
#[cfg(feature = "rest_api")]
pub fn has_targeted_timestamp(target_ip: IpAddr) -> bool {
    let Some(timestamps) = TARGETED_TIMESTAMPS.get() else {
        return false;
    };

    let mut guard = match timestamps.lock() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Warning: Mutex poisoned when checking targeted timestamp: {}", e);
            e.into_inner()
        }
    };

    match guard.get(&target_ip).copied() {
        Some(timestamp) if chrono::Utc::now().timestamp() < timestamp => true,
        Some(_) => {
            guard.remove(&target_ip);
            false
        }
        None => false,
    }
}

/// Returns whether any targeted reservation currently exists.
#[cfg(feature = "rest_api")]
pub fn has_any_targeted_timestamp() -> bool {
    let Some(timestamps) = TARGETED_TIMESTAMPS.get() else {
        return false;
    };

    let mut guard = match timestamps.lock() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Warning: Mutex poisoned when checking targeted timestamps: {}", e);
            e.into_inner()
        }
    };

    let current_time = chrono::Utc::now().timestamp();
    guard.retain(|_, timestamp| *timestamp > current_time);
    !guard.is_empty()
}

/// Removes the targeted reservation for a single configured server IP (if any),
/// aborts its expiration task, and pushes the correct follow-up state to clients
/// at that IP (`CLEAR`, or the current global `SET` if a global plan is active).
///
/// Returns `true` if an entry existed for that IP in the targeted map (removed).
#[cfg(feature = "rest_api")]
pub fn clear_targeted_reservation_for_ip(ip: &str) -> bool {
    let Some(target_ip) = parse_target_ip(ip) else {
        return false;
    };

    let removed = if let Some(timestamps) = TARGETED_TIMESTAMPS.get() {
        let mut guard = match timestamps.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when clearing targeted timestamp: {}", e);
                e.into_inner()
            }
        };
        guard.remove(&target_ip).is_some()
    } else {
        false
    };

    if let Some(tasks) = TARGETED_EXPIRATION_TASKS.get() {
        let mut guard = match tasks.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when canceling targeted expiration task: {}", e);
                e.into_inner()
            }
        };
        if let Some(handle) = guard.remove(&target_ip) {
            handle.abort();
        }
    }

    if removed {
        let msg = if get_current_timestamp().is_some() {
            current_global_message()
        } else {
            "CLEAR".to_string()
        };
        send_targeted_message(target_ip, msg);
    }

    removed
}

/// Clears all targeted reservations and notifies targeted clients.
#[cfg(feature = "rest_api")]
pub fn clear_all_targeted_reservations() {
    let targeted_ips = if let Some(timestamps) = TARGETED_TIMESTAMPS.get() {
        let mut guard = match timestamps.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when clearing targeted timestamps: {}", e);
                e.into_inner()
            }
        };
        let ips = guard.keys().copied().collect::<Vec<_>>();
        guard.clear();
        ips
    } else {
        Vec::new()
    };

    if let Some(tasks) = TARGETED_EXPIRATION_TASKS.get() {
        let mut guard = match tasks.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when clearing targeted expiration tasks: {}", e);
                e.into_inner()
            }
        };
        for (_, handle) in guard.drain() {
            handle.abort();
        }
    }

    for target_ip in targeted_ips {
        send_targeted_message(target_ip, "CLEAR".to_string());
    }
}

/// Registers a WebSocket connection for targeted plan messages.
#[cfg(feature = "rest_api")]
pub fn register_targeted_connection(target_ip: IpAddr) -> (u64, mpsc::UnboundedReceiver<String>) {
    let connection_id = NEXT_CONNECTION_ID
        .get()
        .expect("NEXT_CONNECTION_ID not initialized")
        .fetch_add(1, Ordering::Relaxed);
    let (tx, rx) = mpsc::unbounded_channel();

    let connections = TARGETED_CONNECTIONS
        .get()
        .expect("TARGETED_CONNECTIONS not initialized");
    let mut guard = match connections.lock() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Warning: Mutex poisoned when registering targeted connection: {}", e);
            e.into_inner()
        }
    };

    guard
        .entry(target_ip)
        .or_insert_with(HashMap::new)
        .insert(connection_id, tx);

    (connection_id, rx)
}

/// Unregisters a targeted WebSocket connection.
#[cfg(feature = "rest_api")]
pub fn unregister_targeted_connection(target_ip: IpAddr, connection_id: u64) {
    let Some(connections) = TARGETED_CONNECTIONS.get() else {
        return;
    };

    let mut guard = match connections.lock() {
        Ok(guard) => guard,
        Err(e) => {
            eprintln!("Warning: Mutex poisoned when unregistering targeted connection: {}", e);
            e.into_inner()
        }
    };

    if let Some(senders) = guard.get_mut(&target_ip) {
        senders.remove(&connection_id);
        if senders.is_empty() {
            guard.remove(&target_ip);
        }
    }
}

/// Sets a targeted reservation timestamp for connections matching the provided IP.
#[cfg(feature = "rest_api")]
pub fn set_targeted_reservation_timestamp(ip: &str, timestamp: i64) {
    let Some(target_ip) = parse_target_ip(ip) else {
        eprintln!("Invalid targeted reservation IP: {}", ip);
        return;
    };

    if get_current_timestamp().is_some() {
        clear_reservation();
    }

    if let Some(timestamps) = TARGETED_TIMESTAMPS.get() {
        let mut guard = match timestamps.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when setting targeted timestamp: {}", e);
                e.into_inner()
            }
        };
        guard.insert(target_ip, timestamp);
    }

    if let Some(tasks) = TARGETED_EXPIRATION_TASKS.get() {
        let mut guard = match tasks.lock() {
            Ok(guard) => guard,
            Err(e) => {
                eprintln!("Warning: Mutex poisoned when canceling targeted task: {}", e);
                e.into_inner()
            }
        };
        if let Some(previous_handle) = guard.remove(&target_ip) {
            previous_handle.abort();
        }
    }

    start_targeted_expiration_checker(target_ip, timestamp);
    send_targeted_message(target_ip, format!("SET {}", timestamp));
}

/// Gets a new receiver for the broadcast channel
/// 
/// This function creates a new receiver that can be used to subscribe to
/// timestamp broadcasts. Each WebSocket connection should get its own receiver.
/// 
/// # Returns
/// A new `broadcast::Receiver<String>` that will receive all future broadcasts
/// 
/// # Panics
/// Panics if the broadcaster hasn't been initialized yet. Ensure `init_broadcaster()`
/// is called before using this function.
pub fn get_receiver() -> broadcast::Receiver<String> {
    PLAN_BROADCASTER
        .get()
        .expect("Broadcaster not initialized. Call init_broadcaster() first.")
        .subscribe()
}

