// SPDX-License-Identifier: GPL-3.0-only

#[cfg(feature = "rest_api")]
use rocket::{get, routes};
#[cfg(feature = "rest_api")]
use rocket_ws::{WebSocket, Message};
#[cfg(feature = "rest_api")]
use tokio::sync::broadcast;
#[cfg(feature = "rest_api")]
use rocket::futures::{SinkExt, StreamExt};

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
/// # Ping/Pong Support
/// The server responds to client-initiated ping frames with pong frames containing the same payload,
/// as per the WebSocket protocol (RFC 6455). The server does not initiate pings to avoid issues with
/// network infrastructure that may filter control frames.
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
pub fn plan_timestamp_stream(ws: WebSocket) -> rocket_ws::Channel<'static> {
    ws.channel(move |stream| {
        Box::pin(async move {
            // Split stream into sender and receiver for bidirectional communication
            let (mut ws_sender, mut ws_receiver) = stream.split();
            
            // Get a receiver from the broadcast channel
            let mut rx = crate::steam_bot::plan_broadcast::get_receiver();
            
            // Send welcome message
            ws_sender.send(Message::Text("Connected to plan timestamp stream".into())).await?;
            
            // Send current reservation state if one exists and hasn't expired
            // This ensures clients that connect after a reservation is set will receive it
            if let Some(timestamp) = crate::steam_bot::plan_broadcast::get_current_timestamp() {
                let current_time = chrono::Utc::now().timestamp();
                if current_time < timestamp {
                    // Timestamp is still valid (not expired)
                    ws_sender.send(Message::Text(format!("SET {}", timestamp))).await?;
                } else {
                    // Timestamp has expired, send CLEAR
                    ws_sender.send(Message::Text("CLEAR".into())).await?;
                }
            } else {
                // No reservation set, send CLEAR
                ws_sender.send(Message::Text("CLEAR".into())).await?;
            }
            
            // Listen for broadcasts (SET <timestamp> or CLEAR) and incoming WebSocket messages (pings)
            // Use a timeout on the broadcast receiver to ensure we always poll the stream for pings
            loop {
                tokio::select! {
                    // Handle incoming WebSocket messages (including ping frames)
                    // Server only responds to client pings, does not initiate them
                    msg_result = ws_receiver.next() => {
                        match msg_result {
                            Some(Ok(msg)) => {
                                match msg {
                                    Message::Ping(payload) => {
                                        // Respond to client ping with pong containing the same payload
                                        if let Err(e) = ws_sender.send(Message::Pong(payload)).await {
                                            eprintln!("Failed to send pong response: {}", e);
                                            break;
                                        }
                                    }
                                    Message::Close(_) => {
                                        // Client closed the connection
                                        break;
                                    }
                                    _ => {
                                        // Ignore other message types (text, binary, pong, frame, etc.)
                                    }
                                }
                            }
                            Some(Err(e)) => {
                                eprintln!("WebSocket receive error: {}", e);
                                break;
                            }
                            None => {
                                // Connection closed
                                break;
                            }
                        }
                    }
                    // Handle broadcasts from the reservation system with a timeout
                    // This ensures we don't block indefinitely and can poll the stream
                    result = tokio::time::timeout(tokio::time::Duration::from_millis(100), rx.recv()) => {
                        match result {
                            Ok(Ok(message)) => {
                                // Messages are already formatted as "SET <timestamp>" or "CLEAR"
                                if let Err(e) = ws_sender.send(Message::Text(message)).await {
                                    eprintln!("Failed to send broadcast message: {}", e);
                                    break;
                                }
                            }
                            Ok(Err(broadcast::error::RecvError::Closed)) => {
                                // Channel is closed, notify client and exit
                                let _ = ws_sender.send(Message::Text("Connection closed".into())).await;
                                break;
                            }
                            Ok(Err(broadcast::error::RecvError::Lagged(skipped))) => {
                                // Client lagged behind, log warning but continue
                                eprintln!("WebSocket receiver lagged, skipped {} messages", skipped);
                                // Continue receiving - don't break the connection
                            }
                            Err(_) => {
                                // Timeout - this is expected, continue to poll the stream
                                // This ensures we don't block on rx.recv() and can handle pings
                            }
                        }
                    }
                }
            }
            
            Ok(())
        })
    })
}

/// Mounts WebSocket routes for plan timestamp streaming
/// 
/// # Returns
/// A vector of Rocket routes for WebSocket endpoints
#[cfg(feature = "rest_api")]
pub fn mount_plan_ws_routes() -> Vec<rocket::Route> {
    routes![plan_timestamp_stream]
}

