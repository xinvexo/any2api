use any2api_domain::ProviderKind;
use serde_json::{Map, Value};

use crate::{
    OAuthImportedAccount, ProviderError,
    oauth::OAuthTokenMaterial,
    oauth::import::{
        ProviderHint, absolute_expiry, jwt_claim_text, jwt_expiry, nested_object, path_text,
        provider_hint, safe_email, text,
    },
};

pub(crate) fn parse(
    object: &Map<String, Value>,
) -> Result<Option<OAuthImportedAccount>, ProviderError> {
    if provider_hint(object) != ProviderHint::Provider(ProviderKind::Grok) {
        return Ok(None);
    }

    let sources = token_sources(object);
    let access_token = first_text(&sources, &["access_token", "accessToken"]).unwrap_or_default();
    let refresh_token = first_text(&sources, &["refresh_token", "refreshToken"]);
    let id_token = first_text(&sources, &["id_token", "idToken"]);
    let expires_at = absolute_expiry(&sources)?
        .or_else(|| jwt_expiry(&access_token))
        .or_else(|| id_token.as_deref().and_then(jwt_expiry));
    let account_id = first_text(&sources, &["sub", "subject", "account_id", "accountId"])
        .or_else(|| path_text(object, &["account", "id"]))
        .or_else(|| jwt_identity(&id_token, &access_token, "sub"));
    let email = safe_email(
        first_text(&sources, &["email", "email_address"])
            .or_else(|| path_text(object, &["user", "email"]))
            .or_else(|| jwt_identity(&id_token, &access_token, "email")),
    );
    let preferred_label = text(object, &["name"]).or_else(|| email.clone());
    let token = OAuthTokenMaterial::new(
        ProviderKind::Grok,
        access_token,
        refresh_token,
        id_token,
        expires_at,
        account_id,
        email,
    )?;
    Ok(Some(OAuthImportedAccount::new(token, preferred_label)))
}

fn token_sources(object: &Map<String, Value>) -> Vec<&Map<String, Value>> {
    [nested_object(object, "credentials"), Some(object)]
        .into_iter()
        .flatten()
        .collect()
}

fn first_text(objects: &[&Map<String, Value>], keys: &[&str]) -> Option<String> {
    objects.iter().find_map(|object| text(object, keys))
}

fn jwt_identity(id_token: &Option<String>, access_token: &str, claim: &str) -> Option<String> {
    id_token
        .as_deref()
        .into_iter()
        .chain([access_token])
        .find_map(|token| jwt_claim_text(token, &[claim]))
}
