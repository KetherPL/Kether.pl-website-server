// SPDX-License-Identifier: GPL-3.0-only

use tokio::time::Duration;

/// Tokens that indicate connection-related errors in error messages
pub const CONNECTION_ERROR_TOKENS: [&str; 10] = [
    "broken pipe",
    "connection",
    "network",
    "timeout",
    "closed",
    "io error",
    "disconnected",
    "connection reset",
    "unexpected eof",
    "stream closed",
];

/// Maximum number of reconnection attempts before giving up
pub const RECONNECT_RETRY_LIMIT: u32 = 3;

/// Maximum backoff delay in seconds for exponential backoff during reconnection
pub const MAX_BACKOFF_SECONDS: u64 = 60;

/// Checks if an error message indicates a connection-related error
/// 
/// This function examines error messages for common connection error tokens
/// to determine if the error is related to network connectivity issues.
/// 
/// # Arguments
/// * `message` - The error message to check
/// 
/// # Returns
/// `true` if the message contains connection error tokens, `false` otherwise
/// 
/// # Example
/// ```rust
/// let error_msg = "Network error: connection timeout";
/// if is_connection_error(error_msg) {
///     // Handle connection error
/// }
/// ```
pub fn is_connection_error(message: &str) -> bool {
    let lower = message.to_lowercase();
    CONNECTION_ERROR_TOKENS
        .iter()
        .any(|token| lower.contains(token))
}

/// Calculates the backoff delay for reconnection attempts using exponential backoff
/// 
/// The delay increases exponentially with each attempt, up to a maximum of
/// `MAX_BACKOFF_SECONDS` seconds. This prevents overwhelming the Steam servers
/// with rapid reconnection attempts.
/// 
/// # Arguments
/// * `attempt` - The current reconnection attempt number (1-based)
/// 
/// # Returns
/// A `Duration` representing the delay before the next reconnection attempt
/// 
/// # Example
/// ```rust
/// let delay = calculate_backoff_delay(2); // 2 seconds for second attempt
/// tokio::time::sleep(delay).await;
/// ```
pub fn calculate_backoff_delay(attempt: u32) -> Duration {
    let exponent = attempt.saturating_sub(1);
    let seconds = std::cmp::min(1u64 << exponent, MAX_BACKOFF_SECONDS);
    Duration::from_secs(seconds)
}

