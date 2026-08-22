use std::collections::BTreeSet;

use any2api_domain::{ProviderKind, UpstreamModelName};
use http::{HeaderValue, Method, header};
use serde::Deserialize;
use sha2::{Digest as _, Sha256};
use url::Url;

use crate::{
    OAuthRequestPlan, OAuthTokenMaterial, ProviderError, api::OfficialClientVersion,
    oauth::OAuthModelCatalogScope,
};

use super::{identity, oauth};

const MODELS_URL: &str = "https://chatgpt.com/backend-api/codex/models";
const HASHED_PLAN_SCOPE_PREFIX: &str = "hashed_plan_";
const HEX_DIGITS: &[u8; 16] = b"0123456789abcdef";

pub(crate) fn scope(token: &OAuthTokenMaterial) -> Result<OAuthModelCatalogScope, ProviderError> {
    if token.provider() != ProviderKind::Codex {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Codex".into(),
        ));
    }
    let plan = oauth::plan_label(token)
        .ok_or_else(|| ProviderError::InvalidResponse("Codex OAuth plan is unavailable".into()))?;
    plan_scope(&plan)
}

fn plan_scope(plan: &str) -> Result<OAuthModelCatalogScope, ProviderError> {
    let normalized = plan.to_lowercase();
    if !normalized.starts_with(HASHED_PLAN_SCOPE_PREFIX)
        && let Ok(scope) = OAuthModelCatalogScope::new(ProviderKind::Codex, normalized.clone())
    {
        return Ok(scope);
    }
    OAuthModelCatalogScope::new(
        ProviderKind::Codex,
        encode_digest_scope("hashed_plan", &Sha256::digest(normalized.as_bytes())),
    )
}

fn encode_digest_scope(prefix: &str, digest: &[u8]) -> String {
    let mut scope = String::with_capacity(prefix.len() + 1 + digest.len() * 2);
    scope.push_str(prefix);
    scope.push('_');
    for byte in digest {
        scope.push(char::from(HEX_DIGITS[usize::from(byte >> 4)]));
        scope.push(char::from(HEX_DIGITS[usize::from(byte & 0x0f)]));
    }
    scope
}

pub(crate) fn request_plan(
    token: &OAuthTokenMaterial,
    version: &OfficialClientVersion,
) -> Result<OAuthRequestPlan, ProviderError> {
    if token.provider() != ProviderKind::Codex {
        return Err(ProviderError::InvalidCredential(
            "OAuth token provider does not match Codex".into(),
        ));
    }
    let mut url = Url::parse(MODELS_URL)
        .map_err(|error| ProviderError::InvalidEndpoint(error.to_string()))?;
    url.query_pairs_mut()
        .append_pair("client_version", version.as_str());
    let mut headers = oauth::credential_headers(token)?.headers;
    identity::apply_data_defaults(&mut headers, version);
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
}

#[cfg(test)]
mod tests {
    use super::{HASHED_PLAN_SCOPE_PREFIX, parse, request_plan, scope};
    use any2api_domain::ProviderKind;
    use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

    use crate::{OAuthTokenMaterial, api::OfficialClientVersion};

    #[test]
    fn uses_a_non_secret_plan_scope_and_official_catalog_path() {
        let token = token_with_plan("plus");

        assert_eq!(scope(&token).expect("scope").directory_scope(), "plus");
        let version = OfficialClientVersion::new("9.8.7").expect("version");
        let plan = request_plan(&token, &version).expect("catalog plan");
        assert_eq!(plan.url.path(), "/backend-api/codex/models");
        assert_eq!(plan.url.query(), Some("client_version=9.8.7"));
        assert_eq!(plan.headers["chatgpt-account-id"], "account");
        assert_eq!(plan.headers["originator"], "codex_cli_rs");
        assert_eq!(plan.headers["user-agent"], "codex_cli_rs/9.8.7");
    }

    #[test]
    fn derives_each_model_directory_scope_from_the_current_plan() {
        for plan in ["free", "plus", "prolite", "enterprise_2027"] {
            assert_eq!(
                scope(&token_with_plan(plan))
                    .expect("scope")
                    .directory_scope(),
                plan
            );
        }
    }

    #[test]
    fn safely_isolates_new_plan_formats() {
        let future = scope(&token_with_plan("Future.Plan-Pro 2027/全球")).expect("future scope");
        assert!(
            future
                .directory_scope()
                .starts_with(HASHED_PLAN_SCOPE_PREFIX)
        );
    }

    #[test]
    fn keeps_every_model_returned_by_the_catalog() {
        let models = parse(
            br#"{"models":[{"slug":"gpt-z","supported_in_api":true},{"slug":"gpt-a"},{"slug":"chatgpt-only","supported_in_api":false},{"slug":"gpt-z","supported_in_api":true}]}"#,
        )
        .expect("catalog");
        assert_eq!(models, ["chatgpt-only", "gpt-a", "gpt-z"]);
    }

    fn token_with_plan(plan: &str) -> OAuthTokenMaterial {
        let payload = URL_SAFE_NO_PAD.encode(
            serde_json::json!({
                "https://api.openai.com/auth": {
                    "chatgpt_plan_type": plan,
                },
            })
            .to_string(),
        );
        OAuthTokenMaterial::new(
            ProviderKind::Codex,
            "access".into(),
            None,
            Some(format!("header.{payload}.signature")),
            None,
            Some("account".into()),
            None,
        )
        .expect("token")
    }
}
