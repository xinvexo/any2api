use any2api_domain::ProviderKind;
use serde_json::{Map, Value};

use crate::{
    OAuthImportedAccount, ProviderError,
    oauth::OAuthTokenMaterial,
    oauth_import::{
        ProviderHint, absolute_expiry, jwt_claim_text, jwt_expiry, nested_object, path_text,
        provider_hint, safe_email, text,
    },
};

pub(crate) fn parse(
    object: &Map<String, Value>,
) -> Result<Option<OAuthImportedAccount>, ProviderError> {
    match provider_hint(object) {
        ProviderHint::Provider(ProviderKind::Codex) => {}
        ProviderHint::Unknown if looks_like_codex_session(object) => {}
        ProviderHint::Provider(_) | ProviderHint::NonOAuth | ProviderHint::Unknown => {
            return Ok(None);
        }
    }

    let sources = token_sources(object);
    let access_token =
        first_text(&sources, &["access_token", "accessToken", "token"]).unwrap_or_default();
    let refresh_token = first_text(&sources, &["refresh_token", "refreshToken"]);
    let id_token = first_text(&sources, &["id_token", "idToken"]);
    let expires_at = absolute_expiry(&sources)?
        .or_else(|| jwt_expiry(&access_token))
        .or_else(|| id_token.as_deref().and_then(jwt_expiry));
    let account_id = first_text(
        &sources,
        &[
            "chatgpt_account_id",
            "chatgptAccountId",
            "account_id",
            "accountId",
        ],
    )
    .or_else(|| path_text(object, &["account", "id"]))
    .or_else(|| path_text(object, &["account", "account_id"]))
    .or_else(|| path_text(object, &["account", "chatgpt_account_id"]))
    .or_else(|| jwt_identity(&id_token, &access_token, codex_account_claims()));
    let email = safe_email(
        first_text(&sources, &["email", "email_address"])
            .or_else(|| path_text(object, &["user", "email"]))
            .or_else(|| path_text(object, &["account", "email_address"]))
            .or_else(|| jwt_identity(&id_token, &access_token, &[&["email"]])),
    );
    let preferred_label = text(object, &["name"]).or_else(|| email.clone());
    let token = OAuthTokenMaterial::new(
        ProviderKind::Codex,
        access_token,
        refresh_token,
        id_token,
        expires_at,
        account_id,
        email,
    )?;
    Ok(Some(OAuthImportedAccount::new(token, preferred_label)))
}

fn looks_like_codex_session(object: &Map<String, Value>) -> bool {
    nested_object(object, "tokens").is_some()
        || text(
            object,
            &["accessToken", "chatgpt_account_id", "chatgptAccountId"],
        )
        .is_some()
        || path_text(object, &["account", "id"]).is_some()
        || path_text(object, &["account", "account_id"]).is_some()
        || path_text(object, &["account", "chatgpt_account_id"]).is_some()
}

fn token_sources(object: &Map<String, Value>) -> Vec<&Map<String, Value>> {
    [
        nested_object(object, "credentials"),
        nested_object(object, "tokens"),
        Some(object),
    ]
    .into_iter()
    .flatten()
    .collect()
}

fn first_text(objects: &[&Map<String, Value>], keys: &[&str]) -> Option<String> {
    objects.iter().find_map(|object| text(object, keys))
}

fn jwt_identity(
    id_token: &Option<String>,
    access_token: &str,
    paths: &[&[&str]],
) -> Option<String> {
    id_token
        .as_deref()
        .into_iter()
        .chain([access_token])
        .find_map(|token| paths.iter().find_map(|path| jwt_claim_text(token, path)))
}

fn codex_account_claims() -> &'static [&'static [&'static str]] {
    &[
        &["https://api.openai.com/auth", "chatgpt_account_id"],
        &["chatgpt_account_id"],
        &["account_id"],
    ]
}
