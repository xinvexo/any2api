use any2api_domain::{ConfigRevision, OAuthAccount, OAuthAccountId, OAuthProxySelection};
use any2api_provider::api::{
    MAX_OAUTH_IMPORT_ACCOUNTS_PER_DOCUMENT, OAuthImportParseError, OAuthImportedAccount,
    ProviderRegistry, parse_oauth_import_document,
};
use bytes::Bytes;
use thiserror::Error;

use crate::configuration::{ConfigPublishError, ConfigPublisher, OAuthAccountActivation};

use crate::oauth::document;

pub const MAX_OAUTH_IMPORT_ACCOUNTS: usize = MAX_OAUTH_IMPORT_ACCOUNTS_PER_DOCUMENT;

#[derive(Debug)]
pub struct OAuthImportResult {
    revision: ConfigRevision,
    accounts: Vec<OAuthAccount>,
}

impl OAuthImportResult {
    #[must_use]
    pub const fn revision(&self) -> ConfigRevision {
        self.revision
    }

    #[must_use]
    pub fn accounts(&self) -> &[OAuthAccount] {
        &self.accounts
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthImportFailureKind {
    InvalidJson,
    InvalidEnvelope,
    Empty,
    TooManyAccounts,
    UnsupportedAccount,
    AmbiguousAccount,
    InvalidAccount,
}

#[derive(Debug, Error)]
pub enum OAuthImportError {
    #[error("OAuth import did not contain any files")]
    NoFiles,
    #[error("OAuth import contains too many accounts")]
    TooManyAccounts,
    #[error("OAuth import file {file_index} is invalid")]
    InvalidFile {
        file_index: usize,
        account_index: Option<usize>,
        kind: OAuthImportFailureKind,
    },
    #[error("OAuth import activation failed")]
    Activation(#[source] ConfigPublishError),
}

pub(in crate::oauth) async fn publish(
    providers: &ProviderRegistry,
    publisher: &ConfigPublisher,
    files: Vec<Bytes>,
) -> Result<OAuthImportResult, OAuthImportError> {
    if files.is_empty() {
        return Err(OAuthImportError::NoFiles);
    }
    let mut parsed = Vec::new();
    for (file_offset, bytes) in files.iter().enumerate() {
        let file_index = file_offset + 1;
        let accounts = parse_oauth_import_document(providers, bytes)
            .map_err(|error| map_parse_error(file_index, error))?;
        if parsed.len().saturating_add(accounts.len()) > MAX_OAUTH_IMPORT_ACCOUNTS {
            return Err(OAuthImportError::TooManyAccounts);
        }
        parsed.extend(
            accounts
                .into_iter()
                .enumerate()
                .map(|(account_offset, account)| ParsedAccount {
                    file_index,
                    account_index: account_offset + 1,
                    account,
                }),
        );
    }

    let mut ids = Vec::with_capacity(parsed.len());
    let mut activations = Vec::with_capacity(parsed.len());
    for parsed in parsed {
        let ParsedAccount {
            file_index,
            account_index,
            account,
        } = parsed;
        let (token, preferred_label) = account.into_parts();
        let provider = token.provider();
        let driver = providers
            .get(provider)
            .ok_or_else(|| invalid_account(file_index, account_index))?;
        driver
            .oauth_credential_headers(&token, &http::HeaderMap::new())
            .map_err(|_| invalid_account(file_index, account_index))?;
        let routing_profile = driver
            .oauth_routing_profile(&token)
            .map_err(|_| invalid_account(file_index, account_index))?;
        let models = routing_profile
            .models()
            .iter()
            .map(|model| model.as_str().to_owned())
            .collect();
        let oauth_document = document::build_account_document(&token)
            .map_err(|_| invalid_account(file_index, account_index))?;
        let id = OAuthAccountId::new();
        ids.push(id);
        activations.push(OAuthAccountActivation {
            id,
            provider_kind: provider,
            proxy_selection: OAuthProxySelection::Global,
            preferred_label: preferred_label.or_else(|| token.email().map(str::to_owned)),
            safe_account_email: token.email().map(str::to_owned),
            expires_at: token.expires_at(),
            models,
            document: oauth_document,
            token,
        });
    }

    let snapshot = publisher
        .activate_oauth_accounts(activations)
        .await
        .map_err(OAuthImportError::Activation)?;
    let accounts = ids
        .into_iter()
        .map(|id| {
            snapshot
                .oauth_accounts()
                .get(id)
                .cloned()
                .expect("published OAuth import account is present")
        })
        .collect();
    Ok(OAuthImportResult {
        revision: snapshot.revision(),
        accounts,
    })
}

struct ParsedAccount {
    file_index: usize,
    account_index: usize,
    account: OAuthImportedAccount,
}

fn invalid_account(file_index: usize, account_index: usize) -> OAuthImportError {
    OAuthImportError::InvalidFile {
        file_index,
        account_index: Some(account_index),
        kind: OAuthImportFailureKind::InvalidAccount,
    }
}

fn map_parse_error(file_index: usize, error: OAuthImportParseError) -> OAuthImportError {
    let (account_index, kind) = match error {
        OAuthImportParseError::InvalidJson => (None, OAuthImportFailureKind::InvalidJson),
        OAuthImportParseError::InvalidEnvelope => (None, OAuthImportFailureKind::InvalidEnvelope),
        OAuthImportParseError::Empty => (None, OAuthImportFailureKind::Empty),
        OAuthImportParseError::TooManyAccounts => (None, OAuthImportFailureKind::TooManyAccounts),
        OAuthImportParseError::UnsupportedAccount { account_index } => (
            Some(account_index),
            OAuthImportFailureKind::UnsupportedAccount,
        ),
        OAuthImportParseError::AmbiguousAccount { account_index } => (
            Some(account_index),
            OAuthImportFailureKind::AmbiguousAccount,
        ),
        OAuthImportParseError::InvalidAccount { account_index } => {
            (Some(account_index), OAuthImportFailureKind::InvalidAccount)
        }
    };
    OAuthImportError::InvalidFile {
        file_index,
        account_index,
        kind,
    }
}
