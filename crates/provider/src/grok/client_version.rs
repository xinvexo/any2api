use http::{HeaderMap, Method};
use url::Url;

use crate::api::{OfficialClientVersion, OfficialClientVersionRequestPlan, ProviderError};

const LATEST_VERSION_URL: &str = "https://x.ai/cli/stable";

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
    ProviderError::InvalidResponse("Grok stable release version is invalid".into())
}

#[cfg(test)]
mod tests {
    use http::Method;

    use super::{parse, request_plan};

    #[test]
    fn uses_the_official_stable_channel_without_an_application_fingerprint() {
        let plan = request_plan().expect("request plan");
        assert_eq!(plan.method, Method::GET);
        assert_eq!(plan.url.as_str(), "https://x.ai/cli/stable");
        assert!(plan.headers.is_empty());
        assert!(plan.body.is_empty());
    }

    #[test]
    fn parses_trimmed_stable_semver() {
        assert_eq!(parse(b"1.0.5\n").expect("version").as_str(), "1.0.5");
        assert!(parse(b"1.1.0-rc.1").is_err());
        assert!(parse(b"stable").is_err());
    }
}
