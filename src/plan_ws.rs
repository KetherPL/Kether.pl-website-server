// SPDX-License-Identifier: GPL-3.0-only

#[cfg(feature = "rest_api")]
use rocket::{get, routes};
#[cfg(feature = "rest_api")]
use rocket_ws::{WebSocket, Stream};
#[cfg(feature = "rest_api")]
use tokio::sync::broadcast;

/// WebSocket endpoint for receiving plan timestamps
/// 
/// This endpoint allows clients to connect via WebSocket and receive Unix timestamps
/// in real-time whenever the `!plan` command is executed. Timestamps are sent as
/// plain text strings (not JSON).
/// 
/// # Route
/// `GET /api/ws/plan`
/// 
/// # Protocol
/// - On connection, clients receive: "Connected to plan timestamp stream"
/// - Each timestamp is sent as "SET <timestamp>" (e.g., "SET 1234567890")
/// - On channel closure, clients receive: "Connection closed"
/// 
/// # Example Client Usage
/// ```javascript
/// const ws = new WebSocket('ws://localhost:3001/api/ws/plan');
/// ws.onmessage = (event) => {
///     const timestamp = parseInt(event.data);
///     console.log('Received timestamp:', timestamp);
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
        
        // Listen for timestamp broadcasts
        loop {
            tokio::select! {
                result = rx.recv() => {
                    match result {
                        Ok(timestamp) => {
                            // Send timestamp with "SET " prefix for SourceMod plugin compatibility
                            yield format!("SET {}", timestamp).into();
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

