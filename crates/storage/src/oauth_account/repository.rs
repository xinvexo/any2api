use any2api_domain::{ConfigRevision, OAuthAccountDraft, OAuthAccountId, ProviderKind};
use async_trait::async_trait;
use sqlx::SqliteConnection;

use crate::{
    configuration::{StoredConfiguration, bump_revision, load_configuration_from},
    error::StorageError,
    settings::prune_model_allowlist,
    sqlite::SqliteStore,
    vault::SecretVault,
};

use super::{
    create::OAuthAccountCreate,
    document::OAuthAccountDocument,
    mutation::{OAuthAccountMutation, prepare_oauth_account_mutation},
    writes::execute_oauth_account_change,
};

#[async_trait]
pub trait OAuthAccountRepository: Send + Sync {
    async fn create_oauth_accounts(
        &self,
        expected: ConfigRevision,
        accounts: Vec<OAuthAccountCreate>,
    ) -> Result<StoredConfiguration, StorageError>;

    #[allow(clippy::too_many_arguments)]
    async fn create_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        provider_kind: ProviderKind,
        draft: OAuthAccountDraft,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        models: Vec<String>,
        document: OAuthAccountDocument,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn update_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_config_version: u64,
        draft: OAuthAccountDraft,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn set_oauth_account_models(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_config_version: u64,
        models: Vec<String>,
    ) -> Result<StoredConfiguration, StorageError>;

    #[allow(clippy::too_many_arguments)]
    async fn refresh_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_token_version: u64,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        document: OAuthAccountDocument,
    ) -> Result<StoredConfiguration, StorageError>;

    async fn delete_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_config_version: u64,
    ) -> Result<StoredConfiguration, StorageError>;
}

#[async_trait]
impl OAuthAccountRepository for SqliteStore {
    async fn create_oauth_accounts(
        &self,
        expected: ConfigRevision,
        accounts: Vec<OAuthAccountCreate>,
    ) -> Result<StoredConfiguration, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let (configuration, changed) =
            mutate_create_batch(&mut transaction, self.secret_vault(), expected, accounts).await?;
        if changed {
            transaction.commit().await?;
        } else {
            transaction.rollback().await?;
        }
        Ok(configuration)
    }

    async fn create_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        provider_kind: ProviderKind,
        draft: OAuthAccountDraft,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        models: Vec<String>,
        document: OAuthAccountDocument,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_oauth_account(
            expected,
            OAuthAccountMutation::Create {
                id,
                provider_kind,
                draft,
                safe_account_email,
                expires_at,
                models,
                document,
            },
        )
        .await
    }

    async fn update_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_config_version: u64,
        draft: OAuthAccountDraft,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_oauth_account(
            expected,
            OAuthAccountMutation::Update {
                id,
                expected_config_version,
                draft,
            },
        )
        .await
    }

    async fn set_oauth_account_models(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_config_version: u64,
        models: Vec<String>,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_oauth_account(
            expected,
            OAuthAccountMutation::SetModels {
                id,
                expected_config_version,
                models,
            },
        )
        .await
    }

    async fn refresh_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_token_version: u64,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        document: OAuthAccountDocument,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_oauth_account(
            expected,
            OAuthAccountMutation::Refresh {
                id,
                expected_token_version,
                safe_account_email,
                expires_at,
                document,
            },
        )
        .await
    }

    async fn delete_oauth_account(
        &self,
        expected: ConfigRevision,
        id: OAuthAccountId,
        expected_config_version: u64,
    ) -> Result<StoredConfiguration, StorageError> {
        self.mutate_oauth_account(
            expected,
            OAuthAccountMutation::Delete {
                id,
                expected_config_version,
            },
        )
        .await
    }
}

impl SqliteStore {
    async fn mutate_oauth_account(
        &self,
        expected: ConfigRevision,
        mutation: OAuthAccountMutation,
    ) -> Result<StoredConfiguration, StorageError> {
        let mut transaction = self.pool().begin_with("BEGIN IMMEDIATE").await?;
        let (configuration, changed) =
            mutate_connection(&mut transaction, self.secret_vault(), expected, mutation).await?;
        if changed {
            transaction.commit().await?;
        } else {
            transaction.rollback().await?;
        }
        Ok(configuration)
    }
}

async fn mutate_connection(
    connection: &mut SqliteConnection,
    vault: &SecretVault,
    expected: ConfigRevision,
    mutation: OAuthAccountMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection, vault).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) =
        prepare_oauth_account_mutation(current.oauth_accounts(), current.proxies(), mutation)?
    else {
        return Ok((current, false));
    };
    execute_oauth_account_change(connection, prepared.change()).await?;
    let expected_accounts = prepared.into_configuration();
    prune_model_allowlist(
        connection,
        current.settings(),
        current.model_routes(),
        &expected_accounts,
    )
    .await?;
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection, vault).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.oauth_accounts(), &expected_accounts);
    Ok((configuration, true))
}

async fn mutate_create_batch(
    connection: &mut SqliteConnection,
    vault: &SecretVault,
    expected: ConfigRevision,
    accounts: Vec<OAuthAccountCreate>,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection, vault).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    if accounts.is_empty() {
        return Ok((current, false));
    }

    let mut expected_accounts = current.oauth_accounts().clone();
    let mut changes = Vec::with_capacity(accounts.len());
    for account in accounts {
        let prepared = prepare_oauth_account_mutation(
            &expected_accounts,
            current.proxies(),
            OAuthAccountMutation::Create {
                id: account.id,
                provider_kind: account.provider_kind,
                draft: account.draft,
                safe_account_email: account.safe_account_email,
                expires_at: account.expires_at,
                models: account.models,
                document: account.document,
            },
        )?
        .expect("OAuth account creation always changes the configuration");
        let (next_accounts, change) = prepared.into_parts();
        expected_accounts = next_accounts;
        changes.push(change);
    }

    for change in &changes {
        execute_oauth_account_change(connection, change).await?;
    }
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection, vault).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.oauth_accounts(), &expected_accounts);
    Ok((configuration, true))
}
