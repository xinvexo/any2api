use http::{HeaderMap, Method};
use serde::Deserialize;
use url::Url;

use crate::api::{OfficialClientVersion, OfficialClientVersionRequestPlan, ProviderError};

const LATEST_VERSION_URL: &str = "https://releases.openai.com/codex/channels/latest";

pub(super) fn request_plan() -> Result<OfficialClientVersionRequestPlan, ProviderError> {
    Ok(OfficialClientVersionRequestPlan {
        method: Method::GET,
        url: Url::parse(LATEST_VERSION_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers: HeaderMap::new(),
        body: Vec::new(),
    })
}

pub(super) fn parse(body: &[u8]) -> Result<OfficialClientVersion, ProviderError> {
    let payload = serde_json::from_slice::<LatestRelease>(body)
        .map_err(|_| invalid_response("Codex latest release response is invalid"))?;
    let version = payload
        .tag_name
        .strip_prefix("rust-v")
        .ok_or_else(|| invalid_response("Codex latest release tag is invalid"))?;
    OfficialClientVersion::new(version)
        .map_err(|_| invalid_response("Codex latest release version is invalid"))
}

#[derive(Deserialize)]
struct LatestRelease {
    tag_name: String,
}

fn invalid_response(message: &'static str) -> ProviderError {
    ProviderError::InvalidResponse(message.into())
}

#[cfg(test)]
mod tests {
    use http::Method;

    use super::{parse, request_plan};

    #[test]
    fn uses_the_official_release_channel_without_an_application_fingerprint() {
        let plan = request_plan().expect("request plan");
        assert_eq!(plan.method, Method::GET);
        assert_eq!(
            plan.url.as_str(),
            "https://releases.openai.com/codex/channels/latest"
        );
        assert!(plan.headers.is_empty());
        assert!(plan.body.is_empty());
    }

    #[test]
    fn parses_the_stable_rust_release_tag() {
        assert_eq!(
            parse(br#"{"tag_name":"rust-v0.149.0"}"#)
                .expect("version")
                .as_str(),
            "0.149.0"
        );
        assert!(parse(br#"{"tag_name":"v0.149.0"}"#).is_err());
        assert!(parse(br#"{"tag_name":"rust-v0.150.0-beta.1"}"#).is_err());
    }
}
