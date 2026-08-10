//! Versioned Claude application identity profile.

use http::{HeaderMap, HeaderValue, header};

use crate::header_policy::insert_default;

pub(super) const USER_AGENT: &str = "claude-code/2.1.220";
const APP: &str = "cli";
const API_VERSION: &str = "2023-06-01";
const OAUTH_BETA: &str = "oauth-2025-04-20";

pub(super) fn apply_data_defaults(headers: &mut HeaderMap) {
    insert_default(headers, "user-agent", USER_AGENT);
    insert_default(headers, "x-app", APP);
    insert_default(headers, "anthropic-version", API_VERSION);
}

pub(super) fn apply_quota_defaults(headers: &mut HeaderMap) {
    headers.insert(
        header::ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::USER_AGENT, HeaderValue::from_static(USER_AGENT));
    headers.insert("anthropic-beta", HeaderValue::from_static(OAUTH_BETA));
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;

    use super::{apply_data_defaults, apply_quota_defaults};

    #[test]
    fn data_and_quota_share_one_frozen_claude_code_user_agent() {
        let mut data = HeaderMap::new();
        apply_data_defaults(&mut data);
        let mut quota = HeaderMap::new();
        apply_quota_defaults(&mut quota);

        assert_eq!(data["user-agent"], quota["user-agent"]);
        assert_eq!(data["user-agent"], "claude-code/2.1.220");
    }
}
