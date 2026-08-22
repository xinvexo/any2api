//! Versioned Grok application identity profile.

use http::{HeaderMap, HeaderValue, header};

use crate::{api::OfficialClientVersion, header_policy::insert_default};

pub(super) const CLIENT_IDENTIFIER: &str = "grok-shell";
const CLIENT_MODE: &str = "interactive";

fn user_agent(version: &OfficialClientVersion) -> HeaderValue {
    HeaderValue::from_str(&user_agent_text(version))
        .expect("stable SemVer and build target produce a valid Grok User-Agent")
}

pub(super) fn user_agent_text(version: &OfficialClientVersion) -> String {
    format!(
        "{CLIENT_IDENTIFIER}/{version} ({}; {})",
        std::env::consts::OS,
        std::env::consts::ARCH
    )
}

pub(super) fn apply_data_defaults(
    headers: &mut HeaderMap,
    oauth: bool,
    version: &OfficialClientVersion,
) {
    apply_cli_defaults(headers, version);
    if oauth {
        insert_default(headers, "x-grok-client-mode", CLIENT_MODE);
        apply_oauth_identity(headers);
    }
}

pub(super) fn apply_quota_defaults(headers: &mut HeaderMap, version: &OfficialClientVersion) {
    apply_cli_defaults(headers, version);
    insert_default(headers, "x-grok-client-mode", CLIENT_MODE);
    apply_oauth_identity(headers);
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
}

fn apply_cli_defaults(headers: &mut HeaderMap, version: &OfficialClientVersion) {
    if !headers.contains_key(header::USER_AGENT) {
        headers.insert(header::USER_AGENT, user_agent(version));
    }
    if !headers.contains_key("x-grok-client-version") {
        headers.insert(
            "x-grok-client-version",
            HeaderValue::from_str(version.as_str())
                .expect("stable SemVer produces a valid Grok version header"),
        );
    }
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
    use crate::api::OfficialClientVersion;

    #[test]
    fn data_and_quota_share_target_accurate_cli_identity() {
        let mut data = HeaderMap::new();
        let version = OfficialClientVersion::new("9.8.7").expect("version");
        apply_data_defaults(&mut data, true, &version);
        let mut quota = HeaderMap::new();
        apply_quota_defaults(&mut quota, &version);

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
        assert_eq!(data["x-grok-client-version"], "9.8.7");
        assert_eq!(data["user-agent"], user_agent_text(&version));
        assert!(user_agent_text(&version).contains(std::env::consts::OS));
        assert!(user_agent_text(&version).contains(std::env::consts::ARCH));
    }
}
