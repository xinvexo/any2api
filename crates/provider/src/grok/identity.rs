//! Versioned Grok application identity profile.

use std::sync::LazyLock;

use http::{HeaderMap, HeaderValue, header};

use crate::header_policy::insert_default;

pub(super) const CLIENT_VERSION: &str = "0.2.112";
pub(super) const CLIENT_IDENTIFIER: &str = "grok-shell";
const CLIENT_MODE: &str = "interactive";

static USER_AGENT_TEXT: LazyLock<String> = LazyLock::new(|| {
    format!(
        "{CLIENT_IDENTIFIER}/{CLIENT_VERSION} ({}; {})",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
});
static USER_AGENT_VALUE: LazyLock<HeaderValue> = LazyLock::new(|| {
    HeaderValue::from_str(&USER_AGENT_TEXT).expect("build target produces a valid User-Agent")
});

pub(super) fn apply_data_defaults(headers: &mut HeaderMap, oauth: bool) {
    apply_cli_defaults(headers);
    if oauth {
        insert_default(headers, "x-grok-client-mode", CLIENT_MODE);
        apply_oauth_identity(headers);
    }
}

pub(super) fn apply_quota_defaults(headers: &mut HeaderMap) {
    apply_cli_defaults(headers);
    insert_default(headers, "x-grok-client-mode", CLIENT_MODE);
    apply_oauth_identity(headers);
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
}

#[cfg(test)]
pub(super) fn user_agent_text() -> &'static str {
    &USER_AGENT_TEXT
}

fn apply_cli_defaults(headers: &mut HeaderMap) {
    if !headers.contains_key(header::USER_AGENT) {
        headers.insert(header::USER_AGENT, USER_AGENT_VALUE.clone());
    }
    insert_default(headers, "x-grok-client-version", CLIENT_VERSION);
    insert_default(headers, "x-grok-client-identifier", CLIENT_IDENTIFIER);
}

fn apply_oauth_identity(headers: &mut HeaderMap) {
    headers.insert("x-xai-token-auth", HeaderValue::from_static("xai-grok-cli"));
    headers.insert(
        "x-authenticateresponse",
        HeaderValue::from_static("authenticate-response"),
    );
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;

    use super::{apply_data_defaults, apply_quota_defaults, user_agent_text};

    #[test]
    fn data_and_quota_share_target_accurate_cli_identity() {
        let mut data = HeaderMap::new();
        apply_data_defaults(&mut data, true);
        let mut quota = HeaderMap::new();
        apply_quota_defaults(&mut quota);

        assert_eq!(data["user-agent"], quota["user-agent"]);
        assert_eq!(
            data["x-grok-client-version"],
            quota["x-grok-client-version"]
        );
        assert_eq!(
            data["x-grok-client-identifier"],
            quota["x-grok-client-identifier"]
        );
        assert_eq!(data["x-grok-client-mode"], quota["x-grok-client-mode"]);
        assert_eq!(data["user-agent"], user_agent_text());
        assert!(user_agent_text().contains(std::env::consts::OS));
        assert!(user_agent_text().contains(std::env::consts::ARCH));
    }
}
