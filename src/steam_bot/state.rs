// SPDX-License-Identifier: GPL-3.0-only

/// Connection state for tracking Steam connection health
/// 
/// This enum represents the various states that the SteamBot connection
/// can be in during its lifecycle. It's used to track connection health
/// and manage reconnection logic.
#[derive(Debug, Clone, PartialEq)]
pub enum ConnectionState {
    /// Not connected to Steam
    Disconnected,
    /// Currently attempting to connect
    Connecting,
    /// Successfully connected and authenticated
    Connected,
    /// Connection lost, attempting to reconnect
    Reconnecting,
    /// Connection failed and cannot be recovered
    Failed,
}

