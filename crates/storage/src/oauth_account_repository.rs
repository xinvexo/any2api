use any2api_domain::{ConfigRevision, OAuthAccountDraft, OAuthAccountId, ProviderKind};
use async_trait::async_trait;
use sqlx::SqliteConnection;

use crate::{
    configuration::StoredConfiguration,
    error::StorageError,
    oauth_account_document::OAuthAccountDocument,
    oauth_account_mutation::{OAuthAccountMutation, prepare_oauth_account_mutation},
    oauth_account_writes::execute_oauth_account_change,
    proxy_repository::bump_revision,
    proxy_rows::load_configuration_from,
    sqlite::SqliteStore,
    vault::SecretVault,
};

#[async_trait]
pub trait OAuthAccountRepository: Send + Sync {
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
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection, vault).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.oauth_accounts(), &expected_accounts);
    Ok((configuration, true))
}
