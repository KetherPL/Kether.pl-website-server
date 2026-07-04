// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rand::RngCore;

const CODE_TTL: Duration = Duration::from_secs(120);

struct ExchangeEntry {
	steam_id: i64,
	expires_at: Instant,
}

struct ConsumedEntry {
	steam_id: i64,
	expires_at: Instant,
}

/// In-memory store for single-use OAuth exchange codes (short TTL).
#[derive(Clone)]
pub struct ExchangeCodeStore {
	pending: Arc<Mutex<HashMap<String, ExchangeEntry>>>,
	consumed: Arc<Mutex<HashMap<String, ConsumedEntry>>>,
}

impl ExchangeCodeStore {
	pub fn new() -> Self {
		Self {
			pending: Arc::new(Mutex::new(HashMap::new())),
			consumed: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	/// Issue a single-use exchange code for the given Steam ID.
	pub fn issue_code(&self, steam_id: i64) -> String {
		let mut store = self.pending.lock().expect("exchange store lock poisoned");
		purge_pending(&mut store);

		let mut bytes = [0u8; 32];
		rand::thread_rng().fill_bytes(&mut bytes);
		let code: String = bytes.iter().map(|b| format!("{:02x}", b)).collect();

		store.insert(
			code.clone(),
			ExchangeEntry {
				steam_id,
				expires_at: Instant::now() + CODE_TTL,
			},
		);

		code
	}

	/// Consume a code and return the Steam ID if valid.
	/// Replays the same code within the TTL window (handles duplicate client requests).
	pub fn consume_code(&self, code: &str) -> Option<i64> {
		let code = code.trim();
		if code.is_empty() {
			return None;
		}

		{
			let mut consumed = self.consumed.lock().expect("exchange consumed lock poisoned");
			purge_consumed(&mut consumed);
			if let Some(entry) = consumed.get(code)
				&& entry.expires_at > Instant::now()
			{
				return Some(entry.steam_id);
			}
		}

		let mut store = self.pending.lock().expect("exchange store lock poisoned");
		purge_pending(&mut store);

		let entry = store.remove(code)?;
		if entry.expires_at <= Instant::now() {
			return None;
		}

		let steam_id = entry.steam_id;
		let mut consumed = self.consumed.lock().expect("exchange consumed lock poisoned");
		purge_consumed(&mut consumed);
		consumed.insert(
			code.to_string(),
			ConsumedEntry {
				steam_id,
				expires_at: Instant::now() + CODE_TTL,
			},
		);

		Some(steam_id)
	}

	#[cfg(test)]
	fn insert_expired_for_test(&self, code: &str, steam_id: i64) {
		let mut store = self.pending.lock().expect("exchange store lock poisoned");
		store.insert(
			code.to_string(),
			ExchangeEntry {
				steam_id,
				expires_at: Instant::now() - Duration::from_secs(1),
			},
		);
	}
}

fn purge_pending(store: &mut HashMap<String, ExchangeEntry>) {
	let now = Instant::now();
	store.retain(|_, entry| entry.expires_at > now);
}

fn purge_consumed(store: &mut HashMap<String, ConsumedEntry>) {
	let now = Instant::now();
	store.retain(|_, entry| entry.expires_at > now);
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn issue_and_consume_code() {
		let store = ExchangeCodeStore::new();
		let code = store.issue_code(76561198000000000);
		assert_eq!(store.consume_code(&code), Some(76561198000000000));
	}

	#[test]
	fn consume_is_idempotent_within_ttl() {
		let store = ExchangeCodeStore::new();
		let code = store.issue_code(76561198000000001);
		assert_eq!(store.consume_code(&code), Some(76561198000000001));
		assert_eq!(store.consume_code(&code), Some(76561198000000001));
	}

	#[test]
	fn consume_unknown_code_returns_none() {
		let store = ExchangeCodeStore::new();
		assert_eq!(store.consume_code("nonexistent"), None);
	}

	#[test]
	fn expired_code_is_rejected() {
		let store = ExchangeCodeStore::new();
		store.insert_expired_for_test("expired-code", 76561198000000002);
		assert_eq!(store.consume_code("expired-code"), None);
	}

	#[test]
	fn consume_trims_whitespace() {
		let store = ExchangeCodeStore::new();
		let code = store.issue_code(76561198000000003);
		assert_eq!(store.consume_code(&format!("  {code}  ")), Some(76561198000000003));
	}
}
