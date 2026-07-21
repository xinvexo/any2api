use any2api_domain::{ConfigRevision, ProxyProfile, ProxyProfileId};
use sqlx::SqliteConnection;

use crate::{
    configuration::StoredConfiguration,
    error::StorageError,
    proxy_auth_writes,
    proxy_password::validate,
    proxy_repository::bump_revision,
    proxy_rows::load_configuration_from,
    sqlite::SqliteStore,
    vault::{SecretBytes, SecretContext, SecretVault},
};

pub(crate) enum ProxyAuthenticationMutation {
    Set {
        id: ProxyProfileId,
        username: String,
        password: SecretBytes,
    },
    Clear {
        id: ProxyProfileId,
    },
}

impl SqliteStore {
    pub(crate) async fn mutate_proxy_authentication(
        &self,
        expected: ConfigRevision,
        mutation: ProxyAuthenticationMutation,
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
    mutation: ProxyAuthenticationMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection, vault).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let updated = match mutation {
        ProxyAuthenticationMutation::Set {
            id,
            username,
            password,
        } => {
            let existing = editable_proxy(&current, id)?;
            validate(&password)?;
            let updated = existing.set_authentication(username)?;
            let envelope = vault.seal(
                SecretContext::proxy_password(id, updated.authentication_version()),
                &password,
            )?;
            proxy_auth_writes::set_authentication(connection, &updated, &envelope).await?;
            updated
        }
        ProxyAuthenticationMutation::Clear { id } => {
            let existing = editable_proxy(&current, id)?;
            let updated = existing.clear_authentication()?;
            if &updated == existing {
                return Ok((current, false));
            }
            proxy_auth_writes::clear_authentication(connection, &updated).await?;
            updated
        }
    };
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection, vault).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.proxies().get(updated.id()), Some(&updated));
    Ok((configuration, true))
}

fn editable_proxy(
    configuration: &StoredConfiguration,
    id: ProxyProfileId,
) -> Result<&ProxyProfile, StorageError> {
    let profile = configuration
        .proxies()
        .get(id)
        .ok_or(StorageError::ProxyNotFound(id))?;
    if profile.is_built_in() {
        return Err(StorageError::ProxyProtected);
    }
    Ok(profile)
}
