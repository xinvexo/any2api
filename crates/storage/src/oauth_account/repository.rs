use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::{
        StoredConfiguration, bump_revision, ensure_write_matches, load_configuration_from,
        readback_oauth_account_mutation,
    },
    error::{ConfigurationWriteComponent, StorageError},
    settings::prune_model_allowlist,
};

use super::{
    create::OAuthAccountCreate,
    mutation::{OAuthAccountMutation, prepare_oauth_account_mutation},
    refresh::OAuthAccountRefresh,
    writes::execute_oauth_account_change,
};

pub(crate) async fn mutate_connection(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    mutation: OAuthAccountMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
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
    let configuration = readback_oauth_account_mutation(connection, current, true).await?;
    ensure_oauth_write_matches(&configuration, revision, &expected_accounts)?;
    Ok((configuration, true))
}

pub(crate) async fn mutate_create_batch(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    accounts: Vec<OAuthAccountCreate>,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    if accounts.is_empty() {
        return Ok((current, false));
    }

    let created_at = current_timestamp(connection).await?;
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
                created_at: created_at.clone(),
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
    let configuration = readback_oauth_account_mutation(connection, current, false).await?;
    ensure_oauth_write_matches(&configuration, revision, &expected_accounts)?;
    Ok((configuration, true))
}

pub(crate) async fn mutate_refresh_batch(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    refreshes: Vec<OAuthAccountRefresh>,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }

    let mut expected_accounts = current.oauth_accounts().clone();
    let mut changes = Vec::with_capacity(refreshes.len());
    for refresh in refreshes {
        let (id, expected_token_version, safe_account_email, expires_at, document) =
            refresh.into_parts();
        let prepared = prepare_oauth_account_mutation(
            &expected_accounts,
            current.proxies(),
            OAuthAccountMutation::Refresh {
                id,
                expected_token_version,
                safe_account_email,
                expires_at,
                document,
            },
        );
        let prepared = match prepared {
            Ok(Some(prepared)) => prepared,
            Ok(None) => continue,
            Err(
                StorageError::OAuthAccountNotFound(_)
                | StorageError::OAuthAccountTokenVersionConflict { .. },
            ) => continue,
            Err(error) => return Err(error),
        };
        let (next_accounts, change) = prepared.into_parts();
        expected_accounts = next_accounts;
        changes.push(change);
    }
    if changes.is_empty() {
        return Ok((current, false));
    }

    for change in &changes {
        execute_oauth_account_change(connection, change).await?;
    }
    prune_model_allowlist(
        connection,
        current.settings(),
        current.model_routes(),
        &expected_accounts,
    )
    .await?;
    let revision = bump_revision(connection, expected).await?;
    let configuration = readback_oauth_account_mutation(connection, current, true).await?;
    ensure_oauth_write_matches(&configuration, revision, &expected_accounts)?;
    Ok((configuration, true))
}

fn ensure_oauth_write_matches(
    configuration: &StoredConfiguration,
    revision: ConfigRevision,
    expected_accounts: &any2api_domain::OAuthAccountConfiguration,
) -> Result<(), StorageError> {
    ensure_write_matches(
        configuration.revision(),
        revision,
        ConfigurationWriteComponent::Revision,
    )?;
    ensure_write_matches(
        configuration.oauth_accounts(),
        expected_accounts,
        ConfigurationWriteComponent::OAuthAccounts,
    )
}

async fn current_timestamp(connection: &mut SqliteConnection) -> Result<String, StorageError> {
    sqlx::query_scalar("SELECT CURRENT_TIMESTAMP")
        .fetch_one(connection)
        .await
        .map_err(Into::into)
}
