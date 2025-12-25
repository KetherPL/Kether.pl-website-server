// SPDX-License-Identifier: GPL-3.0-only

#[cfg(feature = "rest_api")]
use rocket::{get, routes};
#[cfg(feature = "rest_api")]
use rocket_ws::{WebSocket, Stream};
#[cfg(feature = "rest_api")]
use tokio::sync::broadcast;

/// WebSocket endpoint for receiving plan timestamps and CLEAR messages
/// 
/// This endpoint allows clients to connect via WebSocket and receive reservation updates
/// in real-time whenever the `!plan` command is executed. Messages are sent as
/// plain text strings (not JSON).
/// 
/// # Route
/// `GET /api/ws/plan`
/// 
/// # Protocol
/// - On connection, clients receive: "Connected to plan timestamp stream"
/// - Immediately after connection, clients receive the current state:
///   - "SET <timestamp>" if there's an active reservation (e.g., "SET 1234567890")
///   - "CLEAR" if there's no active reservation
/// - When a reservation is set, clients receive: "SET <timestamp>" (e.g., "SET 1234567890")
/// - When a reservation is cleared (manually or automatically), clients receive: "CLEAR"
/// - On channel closure, clients receive: "Connection closed"
/// 
/// # Example Client Usage
/// ```javascript
/// const ws = new WebSocket('ws://localhost:3001/api/ws/plan');
/// ws.onmessage = (event) => {
///     const message = event.data;
///     if (message.startsWith('SET ')) {
///         const timestamp = parseInt(message.substring(4));
///         console.log('Reservation set:', timestamp);
///     } else if (message === 'CLEAR') {
///         console.log('Reservation cleared');
///     }
/// };
/// ```
#[cfg(feature = "rest_api")]
#[get("/plan")]
pub fn plan_timestamp_stream(_ws: WebSocket) -> Stream!['static] {
    Stream! { _ws =>
        // Get a receiver from the broadcast channel
        let mut rx = crate::steam_bot::plan_broadcast::get_receiver();
        
        // Send welcome message
        yield "Connected to plan timestamp stream".into();
        
        // Send current reservation state if one exists and hasn't expired
        // This ensures clients that connect after a reservation is set will receive it
        if let Some(timestamp) = crate::steam_bot::plan_broadcast::get_current_timestamp() {
            let current_time = chrono::Utc::now().timestamp();
            if current_time < timestamp {
                // Timestamp is still valid (not expired)
                yield format!("SET {}", timestamp).into();
            } else {
                // Timestamp has expired, send CLEAR
                yield "CLEAR".into();
            }
        } else {
            // No reservation set, send CLEAR
            yield "CLEAR".into();
        }
        
        // Listen for broadcasts (SET <timestamp> or CLEAR)
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(message) => {
                            // Messages are already formatted as "SET <timestamp>" or "CLEAR"
                            yield message.into();
                        }
                        Err(broadcast::error::RecvError::Closed) => {
                            // Channel is closed, notify client and exit
                            yield "Connection closed".into();
                            break;
                        }
                        Err(broadcast::error::RecvError::Lagged(skipped)) => {
                            // Client lagged behind, log warning but continue
                            eprintln!("WebSocket receiver lagged, skipped {} messages", skipped);
                            // Continue receiving - don't break the connection
                        }
                    }
                }
            }
        }
    }
}

/// Mounts WebSocket routes for plan timestamp streaming
/// 
/// # Returns
/// A vector of Rocket routes for WebSocket endpoints
#[cfg(feature = "rest_api")]
pub fn mount_plan_ws_routes() -> Vec<rocket::Route> {
    routes![plan_timestamp_stream]
}

