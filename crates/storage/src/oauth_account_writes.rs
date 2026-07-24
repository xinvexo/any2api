use any2api_domain::{OAuthAccount, OAuthAccountId, ProviderKind};
use sqlx::SqliteConnection;

use crate::{
    error::StorageError, oauth_account_document::OAuthAccountDocument,
    oauth_account_mutation::OAuthAccountDatabaseChange,
};

pub(crate) async fn execute_oauth_account_change(
    connection: &mut SqliteConnection,
    change: &OAuthAccountDatabaseChange,
) -> Result<(), StorageError> {
    match change {
        OAuthAccountDatabaseChange::Create { account, document } => {
            insert(connection, account, document).await?;
            replace_models(connection, account).await?;
        }
        OAuthAccountDatabaseChange::Update(account) => update_metadata(connection, account).await?,
        OAuthAccountDatabaseChange::SetModels(account) => {
            update_metadata(connection, account).await?;
            replace_models(connection, account).await?;
        }
        OAuthAccountDatabaseChange::Refresh {
            account,
            expected_token_version,
            document,
        } => refresh(connection, account, *expected_token_version, document).await?,
        OAuthAccountDatabaseChange::Delete(id) => delete(connection, *id).await?,
    }
    Ok(())
}

async fn insert(
    connection: &mut SqliteConnection,
    account: &OAuthAccount,
    document: &OAuthAccountDocument,
) -> Result<(), StorageError> {
    let bytes = document_bytes(document);
    sqlx::query(concat!(
        "INSERT INTO oauth_accounts ",
        "(id, provider_kind, label, label_key, oauth_json, token_version, account_generation, ",
        "config_version, proxy_profile_id, max_concurrency, enabled, safe_account_email, expires_at) ",
        "VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)"
    ))
    .bind(account.id().to_string())
    .bind(provider_kind_text(account.provider_kind()))
    .bind(account.label())
    .bind(account.label_key())
    .bind(bytes)
    .bind(to_i64(account.token_version())?)
    .bind(to_i64(account.account_generation())?)
    .bind(to_i64(account.config_version())?)
    .bind(account.proxy_profile_id().to_string())
    .bind(i64::from(account.max_concurrency().get()))
    .bind(account.enabled())
    .bind(account.safe_account_email())
    .bind(account.expires_at())
    .execute(&mut *connection)
    .await?;
    Ok(())
}

async fn update_metadata(
    connection: &mut SqliteConnection,
    account: &OAuthAccount,
) -> Result<(), StorageError> {
    let result = sqlx::query(concat!(
        "UPDATE oauth_accounts SET label = ?, label_key = ?, max_concurrency = ?, enabled = ?, ",
        "account_generation = ?, config_version = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
    ))
    .bind(account.label())
    .bind(account.label_key())
    .bind(i64::from(account.max_concurrency().get()))
    .bind(account.enabled())
    .bind(to_i64(account.account_generation())?)
    .bind(to_i64(account.config_version())?)
    .bind(account.id().to_string())
    .execute(connection)
    .await?;
    require_single_row(result.rows_affected(), account.id())
}

async fn refresh(
    connection: &mut SqliteConnection,
    account: &OAuthAccount,
    expected_token_version: u64,
    document: &OAuthAccountDocument,
) -> Result<(), StorageError> {
    let bytes = document_bytes(document);
    let result = sqlx::query(concat!(
        "UPDATE oauth_accounts SET oauth_json = ?, token_version = ?, account_generation = ?, ",
        "safe_account_email = ?, expires_at = ?, updated_at = CURRENT_TIMESTAMP ",
        "WHERE id = ? AND token_version = ?"
    ))
    .bind(bytes)
    .bind(to_i64(account.token_version())?)
    .bind(to_i64(account.account_generation())?)
    .bind(account.safe_account_email())
    .bind(account.expires_at())
    .bind(account.id().to_string())
    .bind(to_i64(expected_token_version)?)
    .execute(&mut *connection)
    .await?;
    if result.rows_affected() == 1 {
        Ok(())
    } else {
        let actual =
            sqlx::query_scalar::<_, i64>("SELECT token_version FROM oauth_accounts WHERE id = ?")
                .bind(account.id().to_string())
                .fetch_optional(&mut *connection)
                .await?;
        match actual {
            Some(actual) => Err(StorageError::OAuthAccountTokenVersionConflict {
                expected: expected_token_version,
                actual: u64::try_from(actual).map_err(|_| StorageError::CorruptConfiguration)?,
            }),
            None => Err(StorageError::OAuthAccountNotFound(account.id())),
        }
    }
}

async fn replace_models(
    connection: &mut SqliteConnection,
    account: &OAuthAccount,
) -> Result<(), StorageError> {
    sqlx::query("DELETE FROM oauth_account_models WHERE oauth_account_id = ?")
        .bind(account.id().to_string())
        .execute(&mut *connection)
        .await?;
    for model in account.models() {
        sqlx::query(
            "INSERT INTO oauth_account_models (oauth_account_id, upstream_model) VALUES (?, ?)",
        )
        .bind(account.id().to_string())
        .bind(model.as_str())
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

async fn delete(connection: &mut SqliteConnection, id: OAuthAccountId) -> Result<(), StorageError> {
    let result = sqlx::query("DELETE FROM oauth_accounts WHERE id = ?")
        .bind(id.to_string())
        .execute(connection)
        .await?;
    require_single_row(result.rows_affected(), id)
}

fn document_bytes(document: &OAuthAccountDocument) -> &[u8] {
    document.expose()
}

fn require_single_row(rows_affected: u64, id: OAuthAccountId) -> Result<(), StorageError> {
    if rows_affected == 1 {
        Ok(())
    } else {
        Err(StorageError::OAuthAccountNotFound(id))
    }
}

fn to_i64(value: u64) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::RevisionOverflow)
}

const fn provider_kind_text(kind: ProviderKind) -> &'static str {
    match kind {
        ProviderKind::Codex => "codex",
        ProviderKind::Claude => "claude",
    }
}
