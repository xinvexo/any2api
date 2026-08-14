use thiserror::Error;

use crate::{
    ModelNameValidationError, OAuthAccountId, OAuthProxySelection, ProviderKind, RequestsPerMinute,
    UpstreamModelName,
};

const MAX_ACCOUNT_LABEL_CHARS: usize = 100;
const MAX_SAFE_EMAIL_CHARS: usize = 320;
const MAX_ACCOUNT_VERSION: u64 = u32::MAX as u64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthAccountDraft {
    label: String,
    requests_per_minute: Option<RequestsPerMinute>,
    enabled: bool,
}

impl OAuthAccountDraft {
    pub fn new(
        label: impl Into<String>,
        requests_per_minute: Option<RequestsPerMinute>,
        enabled: bool,
    ) -> Result<Self, OAuthAccountValidationError> {
        Ok(Self {
            label: validate_label(label.into())?,
            requests_per_minute,
            enabled,
        })
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub const fn requests_per_minute(&self) -> Option<RequestsPerMinute> {
        self.requests_per_minute
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthAccount {
    id: OAuthAccountId,
    provider_kind: ProviderKind,
    label: String,
    proxy_selection: OAuthProxySelection,
    requests_per_minute: Option<RequestsPerMinute>,
    enabled: bool,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
    created_at: String,
    token_version: u64,
    account_generation: u64,
    config_version: u64,
    models: Vec<UpstreamModelName>,
}

impl OAuthAccount {
    #[allow(clippy::too_many_arguments)]
    pub fn create(
        id: OAuthAccountId,
        provider_kind: ProviderKind,
        draft: OAuthAccountDraft,
        proxy_selection: OAuthProxySelection,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        created_at: impl Into<String>,
        models: Vec<String>,
    ) -> Result<Self, OAuthAccountValidationError> {
        Self::restore(
            id,
            provider_kind,
            draft,
            proxy_selection,
            safe_account_email,
            expires_at,
            created_at.into(),
            1,
            1,
            1,
            models,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn restore(
        id: OAuthAccountId,
        provider_kind: ProviderKind,
        draft: OAuthAccountDraft,
        proxy_selection: OAuthProxySelection,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        created_at: String,
        token_version: u64,
        account_generation: u64,
        config_version: u64,
        models: Vec<String>,
    ) -> Result<Self, OAuthAccountValidationError> {
        if !provider_kind.supports_oauth() {
            return Err(OAuthAccountValidationError::UnsupportedProvider);
        }
        if !valid_version(token_version)
            || !valid_version(account_generation)
            || !valid_version(config_version)
        {
            return Err(OAuthAccountValidationError::InvalidVersion);
        }
        if expires_at.is_some_and(|value| value < 0) {
            return Err(OAuthAccountValidationError::InvalidExpiry);
        }
        Ok(Self {
            id,
            provider_kind,
            label: draft.label,
            proxy_selection,
            requests_per_minute: draft.requests_per_minute,
            enabled: draft.enabled,
            safe_account_email: validate_safe_email(safe_account_email)?,
            expires_at,
            created_at: validate_created_at(created_at)?,
            token_version,
            account_generation,
            config_version,
            models: validate_models(models)?,
        })
    }

    pub fn updated(
        &self,
        draft: OAuthAccountDraft,
        proxy_selection: OAuthProxySelection,
    ) -> Result<Self, OAuthAccountValidationError> {
        if self.label == draft.label
            && self.requests_per_minute == draft.requests_per_minute
            && self.enabled == draft.enabled
            && self.proxy_selection == proxy_selection
        {
            return Ok(self.clone());
        }
        let account_generation =
            if (!self.enabled && draft.enabled) || self.proxy_selection != proxy_selection {
                next_version(self.account_generation)?
            } else {
                self.account_generation
            };
        Ok(Self {
            label: draft.label,
            requests_per_minute: draft.requests_per_minute,
            enabled: draft.enabled,
            proxy_selection,
            account_generation,
            config_version: next_version(self.config_version)?,
            ..self.clone()
        })
    }

    pub fn with_models(&self, models: Vec<String>) -> Result<Self, OAuthAccountValidationError> {
        let models = validate_models(models)?;
        if self.models == models {
            return Ok(self.clone());
        }
        Ok(Self {
            models,
            config_version: next_version(self.config_version)?,
            ..self.clone()
        })
    }

    pub fn refreshed(
        &self,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
    ) -> Result<Self, OAuthAccountValidationError> {
        if expires_at.is_some_and(|value| value < 0) {
            return Err(OAuthAccountValidationError::InvalidExpiry);
        }
        Ok(Self {
            safe_account_email: validate_safe_email(safe_account_email)?,
            expires_at,
            token_version: next_version(self.token_version)?,
            ..self.clone()
        })
    }

    pub fn reauthorized(
        &self,
        proxy_selection: OAuthProxySelection,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        models: Vec<String>,
    ) -> Result<Self, OAuthAccountValidationError> {
        if expires_at.is_some_and(|value| value < 0) {
            return Err(OAuthAccountValidationError::InvalidExpiry);
        }
        let safe_account_email = validate_safe_email(safe_account_email)?;
        let models = validate_models(models)?;
        let route_changed = self.proxy_selection != proxy_selection;
        let config_changed = route_changed || self.models != models;
        Ok(Self {
            proxy_selection,
            safe_account_email,
            expires_at,
            models,
            token_version: next_version(self.token_version)?,
            account_generation: if route_changed {
                next_version(self.account_generation)?
            } else {
                self.account_generation
            },
            config_version: if config_changed {
                next_version(self.config_version)?
            } else {
                self.config_version
            },
            ..self.clone()
        })
    }

    #[must_use]
    pub const fn id(&self) -> OAuthAccountId {
        self.id
    }

    #[must_use]
    pub const fn provider_kind(&self) -> ProviderKind {
        self.provider_kind
    }

    #[must_use]
    pub fn label(&self) -> &str {
        &self.label
    }

    #[must_use]
    pub fn label_key(&self) -> String {
        self.label.to_lowercase()
    }

    #[must_use]
    pub const fn proxy_selection(&self) -> OAuthProxySelection {
        self.proxy_selection
    }

    #[must_use]
    pub const fn requests_per_minute(&self) -> Option<RequestsPerMinute> {
        self.requests_per_minute
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn safe_account_email(&self) -> Option<&str> {
        self.safe_account_email.as_deref()
    }

    #[must_use]
    pub const fn expires_at(&self) -> Option<i64> {
        self.expires_at
    }

    #[must_use]
    pub fn created_at(&self) -> &str {
        &self.created_at
    }

    #[must_use]
    pub const fn token_version(&self) -> u64 {
        self.token_version
    }

    #[must_use]
    pub const fn account_generation(&self) -> u64 {
        self.account_generation
    }

    #[must_use]
    pub const fn config_version(&self) -> u64 {
        self.config_version
    }

    #[must_use]
    pub fn models(&self) -> &[UpstreamModelName] {
        &self.models
    }

    #[must_use]
    pub fn supports_model(&self, model: &UpstreamModelName) -> bool {
        self.models.binary_search(model).is_ok()
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum OAuthAccountValidationError {
    #[error("provider does not support OAuth accounts")]
    UnsupportedProvider,
    #[error("OAuth account label must not be empty")]
    EmptyLabel,
    #[error("OAuth account label must be trimmed")]
    LabelNotTrimmed,
    #[error("OAuth account label is too long")]
    LabelTooLong,
    #[error("OAuth account version is invalid")]
    InvalidVersion,
    #[error("OAuth account expiry is invalid")]
    InvalidExpiry,
    #[error("OAuth account creation timestamp is invalid")]
    InvalidCreatedAt,
    #[error("OAuth account email is invalid")]
    InvalidEmail,
    #[error("OAuth account id is duplicated")]
    DuplicateId,
    #[error("OAuth account label is duplicated for this provider")]
    DuplicateLabel,
    #[error("OAuth account model name is invalid: {0}")]
    InvalidModel(ModelNameValidationError),
    #[error("OAuth account model is duplicated")]
    DuplicateModel,
    #[error("OAuth account references a missing proxy profile")]
    MissingProxyProfile,
}

fn validate_label(label: String) -> Result<String, OAuthAccountValidationError> {
    if label.trim().is_empty() {
        return Err(OAuthAccountValidationError::EmptyLabel);
    }
    if label.trim() != label {
        return Err(OAuthAccountValidationError::LabelNotTrimmed);
    }
    if label.chars().count() > MAX_ACCOUNT_LABEL_CHARS {
        return Err(OAuthAccountValidationError::LabelTooLong);
    }
    Ok(label)
}

fn validate_safe_email(
    email: Option<String>,
) -> Result<Option<String>, OAuthAccountValidationError> {
    let Some(email) = email else {
        return Ok(None);
    };
    if email.trim() != email
        || email.is_empty()
        || email.chars().count() > MAX_SAFE_EMAIL_CHARS
        || email.chars().any(char::is_control)
    {
        return Err(OAuthAccountValidationError::InvalidEmail);
    }
    Ok(Some(email))
}

fn validate_created_at(value: String) -> Result<String, OAuthAccountValidationError> {
    if value.trim().is_empty() || value.trim() != value || value.chars().any(char::is_control) {
        return Err(OAuthAccountValidationError::InvalidCreatedAt);
    }
    Ok(value)
}

fn validate_models(
    models: Vec<String>,
) -> Result<Vec<UpstreamModelName>, OAuthAccountValidationError> {
    let mut models = models
        .into_iter()
        .map(|model| {
            UpstreamModelName::new(model).map_err(OAuthAccountValidationError::InvalidModel)
        })
        .collect::<Result<Vec<_>, _>>()?;
    models.sort();
    if models.windows(2).any(|pair| pair[0] == pair[1]) {
        return Err(OAuthAccountValidationError::DuplicateModel);
    }
    Ok(models)
}

const fn valid_version(value: u64) -> bool {
    value > 0 && value <= MAX_ACCOUNT_VERSION
}

fn next_version(value: u64) -> Result<u64, OAuthAccountValidationError> {
    let next = value
        .checked_add(1)
        .ok_or(OAuthAccountValidationError::InvalidVersion)?;
    valid_version(next)
        .then_some(next)
        .ok_or(OAuthAccountValidationError::InvalidVersion)
}
