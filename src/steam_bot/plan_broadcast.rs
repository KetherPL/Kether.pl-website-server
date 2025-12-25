// SPDX-License-Identifier: GPL-3.0-only

use tokio::sync::broadcast;
use once_cell::sync::OnceCell;
use std::sync::{Arc, Mutex};

/// Global broadcast channel for plan timestamps and CLEAR messages
/// 
/// This channel allows broadcasting "SET <timestamp>" or "CLEAR" messages from the `!plan` command
/// to all connected WebSocket clients.
static PLAN_BROADCASTER: OnceCell<broadcast::Sender<String>> = OnceCell::new();

/// Stores the last set reservation timestamp
static LAST_TIMESTAMP: OnceCell<Arc<Mutex<Option<i64>>>> = OnceCell::new();

/// Stores the handle for the expiration checker task
static EXPIRATION_TASK: OnceCell<Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>> = OnceCell::new();

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
/// This function clears the stored timestamp, cancels the expiration task,
/// and broadcasts "CLEAR" to all WebSocket clients.
/// 
/// # Note
/// This function gracefully handles the case where the broadcaster hasn't been
/// initialized yet (e.g., if called before Rocket starts). In such cases, it
/// does nothing.
#[cfg(feature = "rest_api")]
pub fn clear_reservation() {
    // Clear stored timestamp
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
    
    // Cancel expiration task
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
    
    // Broadcast "CLEAR" message
    if let Some(tx) = PLAN_BROADCASTER.get() {
        let _ = tx.send("CLEAR".to_string()); // Ignore errors if no receivers
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
    
    // Get the current runtime handle or spawn on a new runtime
    let handle = if let Ok(rt_handle) = tokio::runtime::Handle::try_current() {
        rt_handle.spawn(async move {
            loop {
                // Check if timestamp has passed
                let current_time = chrono::Utc::now().timestamp();
                if current_time >= timestamp {
                    // Timestamp has expired, clear reservation
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
                                }
                            }
                            break;
                        }
                    };
                    
                    // Only clear if this is still the active timestamp
                    if ts_guard.as_ref().map(|&ts| ts == timestamp).unwrap_or(false) {
                        *ts_guard = None;
                        
                        // Broadcast "CLEAR"
                        if let Some(tx) = PLAN_BROADCASTER.get() {
                            let _ = tx.send("CLEAR".to_string());
                        }
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
                
                // Calculate how long to wait: either until timestamp expires, or 60 seconds, whichever is shorter
                let time_until_expiry = timestamp - current_time;
                let wait_duration = if time_until_expiry > 0 && time_until_expiry < 60 {
                    time_until_expiry
                } else {
                    60
                };
                tokio::time::sleep(tokio::time::Duration::from_secs(wait_duration as u64)).await;
            }
        })
    } else {
        // If we're not in a tokio runtime, we can't spawn tasks
        // This should not happen in normal operation, but we handle it gracefully
        eprintln!("Warning: Cannot spawn expiration checker task - not in tokio runtime");
        return;
    };
    
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
}

/// Broadcasts a message to all connected WebSocket clients
/// 
/// This function sends a message (either "SET <timestamp>" or "CLEAR") to all
/// active receivers (WebSocket connections). If no receivers are connected,
/// the message is silently dropped (this is normal behavior for broadcast channels).
/// 
/// # Arguments
/// * `message` - The message to broadcast (as String)
/// 
/// # Note
/// This function gracefully handles the case where the broadcaster hasn't been
/// initialized yet (e.g., if called before Rocket starts). In such cases, it
/// does nothing.
#[cfg(feature = "rest_api")]
pub fn broadcast_message(message: String) {
    if let Some(tx) = PLAN_BROADCASTER.get() {
        let _ = tx.send(message); // Ignore errors if no receivers
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

