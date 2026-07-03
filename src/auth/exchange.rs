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

/// In-memory store for single-use OAuth exchange codes (short TTL).
#[derive(Clone)]
pub struct ExchangeCodeStore {
	inner: Arc<Mutex<HashMap<String, ExchangeEntry>>>,
}

impl ExchangeCodeStore {
	pub fn new() -> Self {
		Self {
			inner: Arc::new(Mutex::new(HashMap::new())),
		}
	}

	/// Issue a single-use exchange code for the given Steam ID.
	pub fn issue_code(&self, steam_id: i64) -> String {
		let mut store = self.inner.lock().expect("exchange store lock poisoned");
		purge_expired(&mut store);

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

	/// Consume a code and return the Steam ID if valid (single-use).
	pub fn consume_code(&self, code: &str) -> Option<i64> {
		let mut store = self.inner.lock().expect("exchange store lock poisoned");
		purge_expired(&mut store);

		let entry = store.remove(code)?;
		if entry.expires_at <= Instant::now() {
			return None;
		}
		Some(entry.steam_id)
	}

	#[cfg(test)]
	fn insert_expired_for_test(&self, code: &str, steam_id: i64) {
		let mut store = self.inner.lock().expect("exchange store lock poisoned");
		store.insert(
			code.to_string(),
			ExchangeEntry {
				steam_id,
				expires_at: Instant::now() - Duration::from_secs(1),
			},
		);
	}
}

fn purge_expired(store: &mut HashMap<String, ExchangeEntry>) {
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
	fn consume_is_single_use() {
		let store = ExchangeCodeStore::new();
		let code = store.issue_code(76561198000000001);
		assert_eq!(store.consume_code(&code), Some(76561198000000001));
		assert_eq!(store.consume_code(&code), None);
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
}
