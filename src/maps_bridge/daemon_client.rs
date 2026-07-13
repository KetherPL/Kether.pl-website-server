// SPDX-License-Identifier: GPL-3.0-only

use reqwest::RequestBuilder;

pub(super) fn with_daemon_auth(
    request: RequestBuilder,
    api_key: Option<&str>,
) -> RequestBuilder {
    match api_key.map(str::trim).filter(|key| !key.is_empty()) {
        Some(key) => request.bearer_auth(key),
        None => request,
    }
}
