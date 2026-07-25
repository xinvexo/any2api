use std::fmt;

use any2api_domain::ProviderKind;
use base64::{
    Engine as _,
    engine::general_purpose::{URL_SAFE, URL_SAFE_NO_PAD},
};
use serde_json::{Map, Value};
use thiserror::Error;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use crate::{ProviderError, ProviderRegistry, oauth::OAuthTokenMaterial};

pub const MAX_OAUTH_IMPORT_ACCOUNTS_PER_DOCUMENT: usize = 2_000;

pub struct OAuthImportedAccount {
    token: OAuthTokenMaterial,
    preferred_label: Option<String>,
}

impl OAuthImportedAccount {
    #[must_use]
    pub fn new(token: OAuthTokenMaterial, preferred_label: Option<String>) -> Self {
        Self {
            token,
            preferred_label: optional_text(preferred_label),
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (OAuthTokenMaterial, Option<String>) {
        (self.token, self.preferred_label)
    }
}

impl fmt::Debug for OAuthImportedAccount {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("OAuthImportedAccount")
            .field("provider", &self.token.provider())
            .field("token", &"[REDACTED]")
            .field("preferred_label_present", &self.preferred_label.is_some())
            .finish()
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum OAuthImportParseError {
    #[error("OAuth import document is not valid JSON")]
    InvalidJson,
    #[error("OAuth import document has an invalid account envelope")]
    InvalidEnvelope,
    #[error("OAuth import document does not contain any accounts")]
    Empty,
    #[error("OAuth import document contains too many accounts")]
    TooManyAccounts,
    #[error("OAuth import account {account_index} is not supported")]
    UnsupportedAccount { account_index: usize },
    #[error("OAuth import account {account_index} matches multiple providers")]
    AmbiguousAccount { account_index: usize },
    #[error("OAuth import account {account_index} is invalid")]
    InvalidAccount { account_index: usize },
}

pub fn parse_oauth_import_document(
    providers: &ProviderRegistry,
    bytes: &[u8],
) -> Result<Vec<OAuthImportedAccount>, OAuthImportParseError> {
    let root =
        serde_json::from_slice::<Value>(bytes).map_err(|_| OAuthImportParseError::InvalidJson)?;
    let entries = account_entries(&root)?;
    if entries.is_empty() {
        return Err(OAuthImportParseError::Empty);
    }
    if entries.len() > MAX_OAUTH_IMPORT_ACCOUNTS_PER_DOCUMENT {
        return Err(OAuthImportParseError::TooManyAccounts);
    }

    entries
        .into_iter()
        .enumerate()
        .map(|(index, object)| parse_account(providers, object, index + 1))
        .collect()
}

fn account_entries(root: &Value) -> Result<Vec<&Map<String, Value>>, OAuthImportParseError> {
    match root {
        Value::Array(entries) => entries.iter().map(require_object).collect(),
        Value::Object(object) => {
            if let Some(accounts) = object.get("accounts") {
                return require_array(accounts)?
                    .iter()
                    .map(require_object)
                    .collect();
            }
            if let Some(data) = object.get("data").and_then(Value::as_object)
                && let Some(accounts) = data.get("accounts")
            {
                return require_array(accounts)?
                    .iter()
                    .map(require_object)
                    .collect();
            }
            Ok(vec![object])
        }
        _ => Err(OAuthImportParseError::InvalidEnvelope),
    }
}

fn parse_account(
    providers: &ProviderRegistry,
    object: &Map<String, Value>,
    account_index: usize,
) -> Result<OAuthImportedAccount, OAuthImportParseError> {
    let mut matched = None;
    for (_, driver) in providers.iter() {
        let parsed = driver
            .parse_oauth_import(object)
            .map_err(|_| OAuthImportParseError::InvalidAccount { account_index })?;
        if let Some(account) = parsed {
            if matched.is_some() {
                return Err(OAuthImportParseError::AmbiguousAccount { account_index });
            }
            matched = Some(account);
        }
    }
    matched.ok_or(OAuthImportParseError::UnsupportedAccount { account_index })
}

fn require_array(value: &Value) -> Result<&[Value], OAuthImportParseError> {
    value
        .as_array()
        .map(Vec::as_slice)
        .ok_or(OAuthImportParseError::InvalidEnvelope)
}

fn require_object(value: &Value) -> Result<&Map<String, Value>, OAuthImportParseError> {
    value
        .as_object()
        .ok_or(OAuthImportParseError::InvalidEnvelope)
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum ProviderHint {
    Provider(ProviderKind),
    NonOAuth,
    Unknown,
}

pub(crate) fn provider_hint(object: &Map<String, Value>) -> ProviderHint {
    let kind = text(object, &["type"]);
    if let Some(provider) = kind.as_deref().and_then(provider_alias) {
        return ProviderHint::Provider(provider);
    }
    let platform = text(object, &["platform"])
        .as_deref()
        .and_then(provider_alias);
    match (platform, kind.as_deref()) {
        (Some(provider), None) => ProviderHint::Provider(provider),
        (Some(provider), Some(kind)) if kind.eq_ignore_ascii_case("oauth") => {
            ProviderHint::Provider(provider)
        }
        (Some(_), Some(_)) => ProviderHint::NonOAuth,
        (None, _) => ProviderHint::Unknown,
    }
}

fn provider_alias(value: &str) -> Option<ProviderKind> {
    if value.eq_ignore_ascii_case("codex") || value.eq_ignore_ascii_case("openai") {
        Some(ProviderKind::Codex)
    } else if value.eq_ignore_ascii_case("claude") || value.eq_ignore_ascii_case("anthropic") {
        Some(ProviderKind::Claude)
    } else if value.eq_ignore_ascii_case("grok") || value.eq_ignore_ascii_case("xai") {
        Some(ProviderKind::Grok)
    } else {
        None
    }
}

pub(crate) fn nested_object<'a>(
    object: &'a Map<String, Value>,
    key: &str,
) -> Option<&'a Map<String, Value>> {
    object.get(key).and_then(Value::as_object)
}

pub(crate) fn text(object: &Map<String, Value>, keys: &[&str]) -> Option<String> {
    keys.iter().find_map(|key| {
        object
            .get(*key)
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .map(str::to_owned)
    })
}

pub(crate) fn safe_email(value: Option<String>) -> Option<String> {
    value.filter(|email| {
        !email.is_empty()
            && email.trim() == email
            && email.chars().count() <= 320
            && !email.chars().any(char::is_control)
    })
}

pub(crate) fn path_text(object: &Map<String, Value>, path: &[&str]) -> Option<String> {
    let (last, parents) = path.split_last()?;
    let mut current = object;
    for key in parents {
        current = nested_object(current, key)?;
    }
    text(current, &[*last])
}

pub(crate) fn absolute_expiry(
    objects: &[&Map<String, Value>],
) -> Result<Option<i64>, ProviderError> {
    for object in objects {
        for key in ["expired", "expires_at", "expiresAt"] {
            let Some(value) = object.get(key) else {
                continue;
            };
            if value.as_str().is_some_and(|value| value.trim().is_empty()) {
                continue;
            }
            let parsed = parse_timestamp(value).ok_or_else(|| {
                ProviderError::InvalidResponse("OAuth import expiry is invalid".into())
            })?;
            if parsed < 0 {
                return Err(ProviderError::InvalidResponse(
                    "OAuth import expiry is invalid".into(),
                ));
            }
            return Ok(Some(parsed));
        }
    }
    Ok(None)
}

fn parse_timestamp(value: &Value) -> Option<i64> {
    if let Some(value) = value.as_i64() {
        return normalize_unix_timestamp(value);
    }
    let text = value.as_str()?.trim();
    text.parse::<i64>()
        .ok()
        .and_then(normalize_unix_timestamp)
        .or_else(|| {
            OffsetDateTime::parse(text, &Rfc3339)
                .ok()
                .map(|value| value.unix_timestamp())
        })
}

fn normalize_unix_timestamp(value: i64) -> Option<i64> {
    if value < 0 {
        return None;
    }
    // Some Codex session files use JavaScript milliseconds instead of Unix seconds.
    Some(if value >= 100_000_000_000 {
        value / 1_000
    } else {
        value
    })
}

pub(crate) fn jwt_claim_text(token: &str, path: &[&str]) -> Option<String> {
    let payload = token.split('.').nth(1)?;
    let bytes = decode_jwt_payload(payload)?;
    let value = serde_json::from_slice::<Value>(&bytes).ok()?;
    path.iter()
        .try_fold(&value, |current, key| current.get(*key))?
        .as_str()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned)
}

pub(crate) fn jwt_expiry(token: &str) -> Option<i64> {
    let payload = token.split('.').nth(1)?;
    let bytes = decode_jwt_payload(payload)?;
    serde_json::from_slice::<Value>(&bytes)
        .ok()?
        .get("exp")?
        .as_i64()
        .filter(|value| *value >= 0)
}

fn decode_jwt_payload(payload: &str) -> Option<Vec<u8>> {
    URL_SAFE_NO_PAD
        .decode(payload)
        .or_else(|_| URL_SAFE.decode(payload))
        .ok()
}

fn optional_text(value: Option<String>) -> Option<String> {
    value.filter(|value| !value.trim().is_empty())
}
