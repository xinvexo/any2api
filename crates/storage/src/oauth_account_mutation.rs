use any2api_domain::{
    OAuthAccount, OAuthAccountConfiguration, OAuthAccountDraft, OAuthAccountId,
    OAuthAccountValidationError, ProviderKind, ProxyConfiguration,
};

use crate::{
    error::StorageError,
    oauth_account_document::{OAuthAccountDocument, OAuthAccountDocumentValidationError},
};

pub(crate) enum OAuthAccountMutation {
    Create {
        id: OAuthAccountId,
        provider_kind: ProviderKind,
        draft: OAuthAccountDraft,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        models: Vec<String>,
        document: OAuthAccountDocument,
    },
    Update {
        id: OAuthAccountId,
        expected_config_version: u64,
        draft: OAuthAccountDraft,
    },
    SetModels {
        id: OAuthAccountId,
        expected_config_version: u64,
        models: Vec<String>,
    },
    Refresh {
        id: OAuthAccountId,
        expected_token_version: u64,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        document: OAuthAccountDocument,
    },
    Delete {
        id: OAuthAccountId,
        expected_config_version: u64,
    },
}

pub(crate) enum OAuthAccountDatabaseChange {
    Create {
        account: OAuthAccount,
        document: OAuthAccountDocument,
    },
    Update(OAuthAccount),
    SetModels(OAuthAccount),
    Refresh {
        account: OAuthAccount,
        expected_token_version: u64,
        document: OAuthAccountDocument,
    },
    Delete(OAuthAccountId),
}

pub(crate) struct PreparedOAuthAccountMutation {
    configuration: OAuthAccountConfiguration,
    change: OAuthAccountDatabaseChange,
}

impl PreparedOAuthAccountMutation {
    pub(crate) const fn change(&self) -> &OAuthAccountDatabaseChange {
        &self.change
    }

    pub(crate) fn into_configuration(self) -> OAuthAccountConfiguration {
        self.configuration
    }
}

pub(crate) fn prepare_oauth_account_mutation(
    current: &OAuthAccountConfiguration,
    proxies: &ProxyConfiguration,
    mutation: OAuthAccountMutation,
) -> Result<Option<PreparedOAuthAccountMutation>, StorageError> {
    match mutation {
        OAuthAccountMutation::Create {
            id,
            provider_kind,
            draft,
            safe_account_email,
            expires_at,
            models,
            document,
        } => create(
            current,
            proxies,
            id,
            provider_kind,
            draft,
            safe_account_email,
            expires_at,
            models,
            document,
        )
        .map(Some),
        OAuthAccountMutation::Update {
            id,
            expected_config_version,
            draft,
        } => update(current, proxies, id, expected_config_version, draft),
        OAuthAccountMutation::SetModels {
            id,
            expected_config_version,
            models,
        } => set_models(current, proxies, id, expected_config_version, models),
        OAuthAccountMutation::Refresh {
            id,
            expected_token_version,
            safe_account_email,
            expires_at,
            document,
        } => refresh(
            current,
            proxies,
            id,
            expected_token_version,
            safe_account_email,
            expires_at,
            document,
        )
        .map(Some),
        OAuthAccountMutation::Delete {
            id,
            expected_config_version,
        } => delete(current, proxies, id, expected_config_version).map(Some),
    }
}

#[allow(clippy::too_many_arguments)]
fn create(
    current: &OAuthAccountConfiguration,
    proxies: &ProxyConfiguration,
    id: OAuthAccountId,
    provider_kind: ProviderKind,
    draft: OAuthAccountDraft,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
    models: Vec<String>,
    document: OAuthAccountDocument,
) -> Result<PreparedOAuthAccountMutation, StorageError> {
    require_document_provider(&document, provider_kind)?;
    let account = OAuthAccount::create(
        id,
        provider_kind,
        draft,
        safe_account_email,
        expires_at,
        models,
    )?;
    let configuration = replace_account(current, proxies, None, Some(account.clone()))?;
    Ok(PreparedOAuthAccountMutation {
        configuration,
        change: OAuthAccountDatabaseChange::Create { account, document },
    })
}

fn update(
    current: &OAuthAccountConfiguration,
    proxies: &ProxyConfiguration,
    id: OAuthAccountId,
    expected_config_version: u64,
    draft: OAuthAccountDraft,
) -> Result<Option<PreparedOAuthAccountMutation>, StorageError> {
    let existing = require_account_version(current, id, expected_config_version)?;
    let updated = existing.updated(draft)?;
    if &updated == existing {
        return Ok(None);
    }
    let configuration = replace_account(current, proxies, Some(id), Some(updated.clone()))?;
    Ok(Some(PreparedOAuthAccountMutation {
        configuration,
        change: OAuthAccountDatabaseChange::Update(updated),
    }))
}

fn set_models(
    current: &OAuthAccountConfiguration,
    proxies: &ProxyConfiguration,
    id: OAuthAccountId,
    expected_config_version: u64,
    models: Vec<String>,
) -> Result<Option<PreparedOAuthAccountMutation>, StorageError> {
    let existing = require_account_version(current, id, expected_config_version)?;
    let updated = existing.with_models(models)?;
    if &updated == existing {
        return Ok(None);
    }
    let configuration = replace_account(current, proxies, Some(id), Some(updated.clone()))?;
    Ok(Some(PreparedOAuthAccountMutation {
        configuration,
        change: OAuthAccountDatabaseChange::SetModels(updated),
    }))
}

#[allow(clippy::too_many_arguments)]
fn refresh(
    current: &OAuthAccountConfiguration,
    proxies: &ProxyConfiguration,
    id: OAuthAccountId,
    expected_token_version: u64,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
    document: OAuthAccountDocument,
) -> Result<PreparedOAuthAccountMutation, StorageError> {
    let existing = current
        .get(id)
        .ok_or(StorageError::OAuthAccountNotFound(id))?;
    if existing.token_version() != expected_token_version {
        return Err(StorageError::OAuthAccountTokenVersionConflict {
            expected: expected_token_version,
            actual: existing.token_version(),
        });
    }
    require_document_provider(&document, existing.provider_kind())?;
    let updated = existing.refreshed(safe_account_email, expires_at)?;
    let configuration = replace_account(current, proxies, Some(id), Some(updated.clone()))?;
    Ok(PreparedOAuthAccountMutation {
        configuration,
        change: OAuthAccountDatabaseChange::Refresh {
            account: updated,
            expected_token_version,
            document,
        },
    })
}

fn delete(
    current: &OAuthAccountConfiguration,
    proxies: &ProxyConfiguration,
    id: OAuthAccountId,
    expected_config_version: u64,
) -> Result<PreparedOAuthAccountMutation, StorageError> {
    require_account_version(current, id, expected_config_version)?;
    let configuration = replace_account(current, proxies, Some(id), None)?;
    Ok(PreparedOAuthAccountMutation {
        configuration,
        change: OAuthAccountDatabaseChange::Delete(id),
    })
}

fn require_account_version(
    current: &OAuthAccountConfiguration,
    id: OAuthAccountId,
    expected_config_version: u64,
) -> Result<&OAuthAccount, StorageError> {
    let account = current
        .get(id)
        .ok_or(StorageError::OAuthAccountNotFound(id))?;
    if account.config_version() != expected_config_version {
        return Err(StorageError::OAuthAccountVersionConflict {
            expected: expected_config_version,
            actual: account.config_version(),
        });
    }
    Ok(account)
}

fn replace_account(
    current: &OAuthAccountConfiguration,
    proxies: &ProxyConfiguration,
    replaced_id: Option<OAuthAccountId>,
    replacement: Option<OAuthAccount>,
) -> Result<OAuthAccountConfiguration, StorageError> {
    let mut accounts = current
        .accounts()
        .iter()
        .filter(|account| Some(account.id()) != replaced_id)
        .cloned()
        .collect::<Vec<_>>();
    accounts.extend(replacement);
    OAuthAccountConfiguration::new(accounts, proxies).map_err(map_configuration_error)
}

fn require_document_provider(
    document: &OAuthAccountDocument,
    provider: ProviderKind,
) -> Result<(), StorageError> {
    if document.provider_kind() == provider {
        Ok(())
    } else {
        Err(OAuthAccountDocumentValidationError::ProviderMismatch.into())
    }
}

fn map_configuration_error(error: OAuthAccountValidationError) -> StorageError {
    match error {
        OAuthAccountValidationError::DuplicateLabel => StorageError::OAuthAccountLabelConflict,
        other => StorageError::OAuthAccountValidation(other),
    }
}
