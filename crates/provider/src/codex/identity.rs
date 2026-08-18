//! Versioned Codex application identity profile.

use http::{HeaderMap, HeaderValue, header};

use crate::header_policy::insert_default;

// This is the Provider fallback when no same-dialect client persona is available.
// It is deliberately not replaced by one entrypoint's current official persona;
// See docs/baselines/official-clients for the independent exec/TUI evidence.
const DATA_ORIGINATOR: &str = "codex_cli_rs";
const DATA_USER_AGENT: &str = "codex_cli_rs/0.145.0";

pub(super) fn apply_data_defaults(headers: &mut HeaderMap) {
    insert_default(headers, "originator", DATA_ORIGINATOR);
    insert_default(headers, "user-agent", DATA_USER_AGENT);
}

pub(super) fn apply_quota_defaults(headers: &mut HeaderMap) {
    for (name, value) in [
        ("openai-beta", "codex-1"),
        ("oai-language", "zh-CN"),
        ("originator", "Codex Desktop"),
        ("sec-fetch-site", "none"),
        ("sec-fetch-mode", "no-cors"),
        ("sec-fetch-dest", "empty"),
        ("priority", "u=4, i"),
    ] {
        headers.insert(
            http::header::HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;

    use super::{apply_data_defaults, apply_quota_defaults};

    #[test]
    fn data_and_wham_quota_subprofiles_are_explicit_and_stable() {
        let mut data = HeaderMap::new();
        apply_data_defaults(&mut data);
        assert_eq!(data["originator"], "codex_cli_rs");
        assert_eq!(data["user-agent"], "codex_cli_rs/0.145.0");

        let mut quota = HeaderMap::new();
        apply_quota_defaults(&mut quota);
        assert_eq!(quota["originator"], "Codex Desktop");
        assert_eq!(quota["openai-beta"], "codex-1");
        assert!(!quota.contains_key("user-agent"));
    }
}
