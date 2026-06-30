// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::hash::Hash;
use std::sync::RwLock;
use std::time::{Duration, Instant};

const WINDOW: Duration = Duration::from_secs(60);

struct SlidingWindowLimiterInner<K> {
	entries: HashMap<K, Vec<Instant>>,
}

impl<K: Eq + Hash> SlidingWindowLimiterInner<K> {
	fn new() -> Self {
		Self {
			entries: HashMap::new(),
		}
	}

	fn check_and_record(&mut self, key: K, limit_per_minute: u32, burst: u32) -> bool {
		let max_requests = limit_per_minute.saturating_add(burst).max(1) as usize;
		let now = Instant::now();
		let cutoff = now.checked_sub(WINDOW).unwrap_or_else(Instant::now);

		let timestamps = self.entries.entry(key).or_default();
		timestamps.retain(|ts| *ts > cutoff);

		if timestamps.len() >= max_requests {
			return false;
		}

		timestamps.push(now);
		true
	}
}

pub struct SlidingWindowLimiter<K> {
	inner: RwLock<SlidingWindowLimiterInner<K>>,
}

impl<K: Eq + Hash + Clone> SlidingWindowLimiter<K> {
	pub fn new() -> Self {
		Self {
			inner: RwLock::new(SlidingWindowLimiterInner::new()),
		}
	}

	pub fn check_and_record(&self, key: K, limit_per_minute: u32, burst: u32) -> bool {
		match self.inner.write() {
			Ok(mut guard) => guard.check_and_record(key, limit_per_minute, burst),
			Err(mut poisoned) => poisoned
				.get_mut()
				.check_and_record(key, limit_per_minute, burst),
		}
	}
}

impl<K: Eq + Hash + Clone> Default for SlidingWindowLimiter<K> {
	fn default() -> Self {
		Self::new()
	}
}

#[cfg(test)]
mod tests {
	use super::*;

	#[test]
	fn allows_up_to_limit_plus_burst() {
		let limiter = SlidingWindowLimiter::new();
		for _ in 0..5 {
			assert!(limiter.check_and_record("client", 2, 3));
		}
		assert!(!limiter.check_and_record("client", 2, 3));
	}
}
