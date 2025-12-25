// SPDX-License-Identifier: GPL-3.0-only

use tokio::sync::broadcast;
use once_cell::sync::OnceCell;

/// Global broadcast channel for plan timestamps
/// 
/// This channel allows broadcasting Unix timestamps from the `!plan` command
/// to all connected WebSocket clients.
static PLAN_BROADCASTER: OnceCell<broadcast::Sender<i64>> = OnceCell::new();

/// Initializes the broadcast channel for plan timestamps
/// 
/// This function should be called once during application startup (typically
/// in the Rocket initialization). It creates a broadcast channel with a
/// buffer capacity of 100 messages.
/// 
/// # Returns
/// A receiver that can be used for testing or debugging purposes
/// 
/// # Panics
/// Panics if called more than once, as the broadcaster can only be initialized once.
pub fn init_broadcaster() -> broadcast::Receiver<i64> {
    let (tx, rx) = broadcast::channel(100);
    PLAN_BROADCASTER.set(tx).expect("Broadcaster already initialized");
    rx
}

/// Broadcasts a Unix timestamp to all connected WebSocket clients
/// 
/// This function sends the timestamp to all active receivers (WebSocket connections).
/// If no receivers are connected, the message is silently dropped (this is normal
/// behavior for broadcast channels).
/// 
/// # Arguments
/// * `timestamp` - The Unix timestamp to broadcast (as i64)
/// 
/// # Note
/// This function gracefully handles the case where the broadcaster hasn't been
/// initialized yet (e.g., if called before Rocket starts). In such cases, it
/// does nothing.
pub fn broadcast_timestamp(timestamp: i64) {
    if let Some(tx) = PLAN_BROADCASTER.get() {
        let _ = tx.send(timestamp); // Ignore errors if no receivers
    }
}

/// Gets a new receiver for the broadcast channel
/// 
/// This function creates a new receiver that can be used to subscribe to
/// timestamp broadcasts. Each WebSocket connection should get its own receiver.
/// 
/// # Returns
/// A new `broadcast::Receiver<i64>` that will receive all future timestamp broadcasts
/// 
/// # Panics
/// Panics if the broadcaster hasn't been initialized yet. Ensure `init_broadcaster()`
/// is called before using this function.
pub fn get_receiver() -> broadcast::Receiver<i64> {
    PLAN_BROADCASTER
        .get()
        .expect("Broadcaster not initialized. Call init_broadcaster() first.")
        .subscribe()
}

