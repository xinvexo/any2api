//! Versioned Claude application identity profile.

use http::{HeaderMap, HeaderValue, header};

use crate::{api::OfficialClientVersion, header_policy::insert_default};

const APP: &str = "cli";
const API_VERSION: &str = "2023-06-01";
const OAUTH_BETA: &str = "oauth-2025-04-20";

pub(super) fn apply_data_defaults(headers: &mut HeaderMap, version: &OfficialClientVersion) {
    insert_user_agent_default(headers, version);
    insert_default(headers, "x-app", APP);
    insert_default(headers, "anthropic-version", API_VERSION);
}

pub(super) fn apply_quota_defaults(headers: &mut HeaderMap, version: &OfficialClientVersion) {
    headers.insert(
        header::ACCEPT,
        HeaderValue::from_static("application/json, text/plain, */*"),
    );
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json"),
    );
    headers.insert(header::USER_AGENT, user_agent(version));
    headers.insert("anthropic-beta", HeaderValue::from_static(OAUTH_BETA));
}

fn insert_user_agent_default(headers: &mut HeaderMap, version: &OfficialClientVersion) {
    if !headers.contains_key(header::USER_AGENT) {
        headers.insert(header::USER_AGENT, user_agent(version));
    }
}

fn user_agent(version: &OfficialClientVersion) -> HeaderValue {
    HeaderValue::from_str(&format!("claude-code/{version}"))
        .expect("stable SemVer produces a valid Claude Code User-Agent")
}

#[cfg(test)]
mod tests {
    use http::HeaderMap;

    use super::{apply_data_defaults, apply_quota_defaults};
    use crate::api::OfficialClientVersion;

    #[test]
    fn data_and_quota_share_one_frozen_claude_code_user_agent() {
        let mut data = HeaderMap::new();
        let version = OfficialClientVersion::new("9.8.7").expect("version");
        apply_data_defaults(&mut data, &version);
        let mut quota = HeaderMap::new();
        apply_quota_defaults(&mut quota, &version);

        assert_eq!(data["user-agent"], quota["user-agent"]);
        assert_eq!(data["user-agent"], "claude-code/9.8.7");
    }
}
