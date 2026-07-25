use any2api_domain::ProviderKind;
use serde_json::{Map, Value};

use crate::{
    OAuthImportedAccount, ProviderError,
    oauth::OAuthTokenMaterial,
    oauth_import::{
        ProviderHint, absolute_expiry, nested_object, path_text, provider_hint, safe_email, text,
    },
};

pub(crate) fn parse(
    object: &Map<String, Value>,
) -> Result<Option<OAuthImportedAccount>, ProviderError> {
    if provider_hint(object) != ProviderHint::Provider(ProviderKind::Claude) {
        return Ok(None);
    }

    let sources = token_sources(object);
    let access_token = first_text(&sources, &["access_token", "accessToken"]).unwrap_or_default();
    let refresh_token = first_text(&sources, &["refresh_token", "refreshToken"]);
    let id_token = first_text(&sources, &["id_token", "idToken"]);
    let expires_at = absolute_expiry(&sources)?;
    let email = safe_email(
        first_text(&sources, &["email", "email_address"])
            .or_else(|| path_text(object, &["account", "email_address"]))
            .or_else(|| path_text(object, &["user", "email"])),
    );
    let preferred_label = text(object, &["name"]).or_else(|| email.clone());
    let token = OAuthTokenMaterial::new(
        ProviderKind::Claude,
        access_token,
        refresh_token,
        id_token,
        expires_at,
        None,
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
