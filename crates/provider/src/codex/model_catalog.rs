use std::collections::BTreeSet;

use any2api_domain::{ProviderKind, UpstreamModelName};
use http::{HeaderValue, Method, header};
use serde::Deserialize;
use url::Url;

use crate::{OAuthRequestPlan, OAuthTokenMaterial, ProviderError, oauth::OAuthModelCatalogScope};

use super::{identity, oauth};

const MODELS_URL: &str = "https://chatgpt.com/backend-api/codex/models";
const CLIENT_VERSION: &str = "0.145.0";

pub(crate) fn scope(token: &OAuthTokenMaterial) -> Result<OAuthModelCatalogScope, ProviderError> {
    let scope = oauth::plan_label(token).as_deref().map_or("free", |plan| {
        if plan.eq_ignore_ascii_case("pro") || plan.eq_ignore_ascii_case("plus") {
            "plus_or_pro"
        } else if plan.eq_ignore_ascii_case("team")
            || plan.eq_ignore_ascii_case("business")
            || plan.eq_ignore_ascii_case("go")
        {
            "team_or_business_or_go"
        } else {
            "free"
        }
    });
    OAuthModelCatalogScope::new(ProviderKind::Codex, scope)
}

pub(crate) fn request_plan(token: &OAuthTokenMaterial) -> Result<OAuthRequestPlan, ProviderError> {
    if token.provider() != ProviderKind::Codex {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Codex".into(),
        ));
    }
    let mut url = Url::parse(MODELS_URL)
        .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("client_version", CLIENT_VERSION);
    let mut headers = oauth::credential_headers(token)?.headers;
    identity::apply_data_defaults(&mut headers);
    headers.insert(header::ACCEPT, HeaderValue::from_static("application/json"));
    Ok(OAuthRequestPlan {
        method: Method::GET,
        url,
        headers,
        body: Vec::new(),
    })
}

pub(crate) fn parse(body: &[u8]) -> Result<Vec<String>, ProviderError> {
    let payload = serde_json::from_slice::<CatalogPayload>(body)
        .map_err(|_| ProviderError::InvalidResponse("Codex model catalog is invalid".into()))?;
    let mut models = BTreeSet::new();
    for model in payload.models {
        if model.supported_in_api == Some(false) {
            continue;
        }
        let model = UpstreamModelName::new(model.slug)
            .map_err(|error| ProviderError::InvalidResponse(error.to_string()))?;
        models.insert(model.as_str().to_owned());
    }
    Ok(models.into_iter().collect())
}

#[derive(Deserialize)]
struct CatalogPayload {
    models: Vec<CatalogModel>,
}

#[derive(Deserialize)]
struct CatalogModel {
    slug: String,
    supported_in_api: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::{parse, request_plan, scope};
    use any2api_domain::ProviderKind;

    use crate::OAuthTokenMaterial;

    #[test]
    fn uses_a_non_secret_plan_scope_and_official_catalog_path() {
        let token = OAuthTokenMaterial::new(
            ProviderKind::Codex,
            "access".into(),
            None,
            Some("header.eyJodHRwczovL2FwaS5vcGVuYWkuY29tL2F1dGgiOnsiY2hhdGdwdF9wbGFuX3R5cGUiOiJwbHVzIn19.signature".into()),
            None,
            Some("account".into()),
            None,
        )
        .expect("token");

        assert_eq!(
            scope(&token).expect("scope").directory_scope(),
            "plus_or_pro"
        );
        let plan = request_plan(&token).expect("catalog plan");
        assert_eq!(plan.url.path(), "/backend-api/codex/models");
        assert_eq!(plan.url.query(), Some("client_version=0.145.0"));
        assert_eq!(plan.headers["chatgpt-account-id"], "account");
        assert_eq!(plan.headers["originator"], "codex_cli_rs");
    }

    #[test]
    fn parses_only_api_supported_models_without_a_local_catalog() {
        let models = parse(
            br#"{"models":[{"slug":"gpt-z","supported_in_api":true},{"slug":"gpt-a"},{"slug":"hidden","supported_in_api":false},{"slug":"gpt-z","supported_in_api":true}]}"#,
        )
        .expect("catalog");
        assert_eq!(models, ["gpt-a", "gpt-z"]);
    }
}
