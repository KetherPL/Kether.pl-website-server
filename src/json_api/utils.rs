// SPDX-License-Identifier: GPL-3.0-only

use rocket::http::Status;

/// Returns HTTP 200 to satisfy CORS preflight handlers.
pub fn ok_status() -> Status {
	Status::Ok
}

/// Logs a storage-layer error with a consistent prefix.
fn log_storage_error(context: &str, error: &str) {
	eprintln!("Error {}: {}", context, error);
}

fn contains_any(haystack: &str, needles: &[&str]) -> bool {
	needles.iter().any(|needle| haystack.contains(needle))
}

/// Maps storage errors to HTTP status codes while logging the failure.
pub fn storage_error_status(
	context: &str,
	error: &str,
	not_found_markers: &[&str],
	conflict_markers: &[&str],
) -> Status {
	log_storage_error(context, error);

	if contains_any(error, not_found_markers) {
		Status::NotFound
	} else if contains_any(error, conflict_markers) {
		Status::Conflict
	} else {
		Status::InternalServerError
	}
}


