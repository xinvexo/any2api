use http::{HeaderMap, Method};
use url::Url;

use crate::api::{OfficialClientVersion, OfficialClientVersionRequestPlan, ProviderError};

const LATEST_VERSION_URL: &str = "https://downloads.claude.ai/claude-code-releases/latest";

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
    let version = std::str::from_utf8(body)
        .map_err(|_| invalid_response())?
        .trim();
    OfficialClientVersion::new(version).map_err(|_| invalid_response())
}

fn invalid_response() -> ProviderError {
    ProviderError::InvalidResponse("Claude latest release version is invalid".into())
}

#[cfg(test)]
mod tests {
    use http::Method;

    use super::{parse, request_plan};

    #[test]
    fn uses_the_official_latest_channel_without_an_application_fingerprint() {
        let plan = request_plan().expect("request plan");
        assert_eq!(plan.method, Method::GET);
        assert_eq!(
            plan.url.as_str(),
            "https://downloads.claude.ai/claude-code-releases/latest"
        );
        assert!(plan.headers.is_empty());
        assert!(plan.body.is_empty());
    }

    #[test]
    fn parses_trimmed_stable_semver() {
        assert_eq!(parse(b"2.1.240\n").expect("version").as_str(), "2.1.240");
        assert!(parse(b"2.2.0-beta.1").is_err());
        assert!(parse(b"latest").is_err());
    }
}
