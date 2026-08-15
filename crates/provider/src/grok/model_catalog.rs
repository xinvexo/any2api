use any2api_domain::ProviderKind;
use http::{HeaderValue, Method, header};
use url::Url;

use crate::{OAuthRequestPlan, OAuthTokenMaterial, ProviderError, oauth::OAuthModelCatalogScope};

use super::{identity, oauth};

const MODELS_URL: &str = "https://cli-chat-proxy.grok.com/v1/models";

pub(crate) fn scope(token: &OAuthTokenMaterial) -> Result<OAuthModelCatalogScope, ProviderError> {
    if token.provider() != ProviderKind::Grok {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Grok".into(),
        ));
    }
    OAuthModelCatalogScope::new(ProviderKind::Grok, "subscription")
}

pub(crate) fn request_plan(token: &OAuthTokenMaterial) -> Result<OAuthRequestPlan, ProviderError> {
    let mut headers = oauth::credential_headers(token)?.headers;
    identity::apply_data_defaults(&mut headers, true);
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
    fn uses_subscription_directory_and_oauth_identity_headers() {
        let token = OAuthTokenMaterial::new(
            ProviderKind::Grok,
            "access".into(),
            None,
            None,
            None,
            Some("subject".into()),
            None,
        )
        .expect("token");
        assert_eq!(
            scope(&token).expect("scope").directory_scope(),
            "subscription"
        );
        let plan = request_plan(&token).expect("catalog plan");
        assert_eq!(
            plan.url.as_str(),
            "https://cli-chat-proxy.grok.com/v1/models"
        );
        assert_eq!(plan.headers["x-userid"], "subject");
        assert_eq!(plan.headers["x-xai-token-auth"], "xai-grok-cli");
        assert_eq!(
            parse(br#"{"data":[{"id":"grok-z"},{"id":"grok-a"}]}"#).expect("catalog"),
            ["grok-a", "grok-z"]
        );
    }
}
