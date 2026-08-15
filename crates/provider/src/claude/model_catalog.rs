use any2api_domain::ProviderKind;
use http::{HeaderMap, HeaderValue, Method, header};
use url::Url;

use crate::{OAuthRequestPlan, OAuthTokenMaterial, ProviderError, oauth::OAuthModelCatalogScope};

use super::oauth;

const MODELS_URL: &str = "https://api.anthropic.com/v1/models";

pub(crate) fn scope(token: &OAuthTokenMaterial) -> Result<OAuthModelCatalogScope, ProviderError> {
    if token.provider() != ProviderKind::Claude {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Claude".into(),
        ));
    }
    OAuthModelCatalogScope::new(ProviderKind::Claude, "subscription")
}

pub(crate) fn request_plan(token: &OAuthTokenMaterial) -> Result<OAuthRequestPlan, ProviderError> {
    let mut headers = oauth::credential_headers(token, &HeaderMap::new())?.headers;
    headers.insert("anthropic-version", HeaderValue::from_static("2023-06-01"));
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    Ok(OAuthRequestPlan {
        method: Method::GET,
        url: Url::parse(MODELS_URL)
            .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?,
        headers,
        body: Vec::new(),
    })
}

pub(crate) fn parse(body: &[u8]) -> Result<Vec<String>, ProviderError> {
    crate::credential::api_key::parse_model_catalog(body)
}

#[cfg(test)]
mod tests {
    use super::{parse, request_plan, scope};
    use any2api_domain::ProviderKind;

    use crate::OAuthTokenMaterial;

    #[test]
    fn uses_the_oauth_models_endpoint_without_a_static_catalog() {
        let token = OAuthTokenMaterial::new(
            ProviderKind::Claude,
            "access".into(),
            None,
            None,
            None,
            None,
            None,
        )
        .expect("token");
        assert_eq!(
            scope(&token).expect("scope").directory_scope(),
            "subscription"
        );
        let plan = request_plan(&token).expect("catalog plan");
        assert_eq!(plan.url.as_str(), "https://api.anthropic.com/v1/models");
        assert_eq!(plan.headers["anthropic-version"], "2023-06-01");
        assert_eq!(plan.headers["anthropic-beta"], "oauth-2025-04-20");
        assert_eq!(plan.headers["authorization"], "Bearer access");
        assert_eq!(
            parse(br#"{"data":[{"id":"claude-z"},{"id":"claude-a"}]}"#).expect("catalog"),
            ["claude-a", "claude-z"]
        );
    }
}
