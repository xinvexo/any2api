use any2api_domain::{ConfigRevision, ProxyProfile, ProxyProfileId};
use sqlx::SqliteConnection;

use crate::{
    configuration::{
        StoredConfiguration, bump_revision, ensure_write_matches, load_configuration_from,
        readback_proxy_mutation,
    },
    error::{ConfigurationWriteComponent, StorageError},
    secret::SecretBytes,
};

use super::{authentication_writes, password::validate};

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

pub(crate) async fn mutate_connection(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    mutation: ProxyAuthenticationMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
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
            authentication_writes::set_authentication(connection, &updated, &password).await?;
            updated
        }
        ProxyAuthenticationMutation::Clear { id } => {
            let existing = editable_proxy(&current, id)?;
            let updated = existing.clear_authentication()?;
            if &updated == existing {
                return Ok((current, false));
            }
            authentication_writes::clear_authentication(connection, &updated).await?;
            updated
        }
    };
    let revision = bump_revision(connection, expected).await?;
    let configuration = readback_proxy_mutation(connection, current).await?;
    ensure_write_matches(
        configuration.revision(),
        revision,
        ConfigurationWriteComponent::Revision,
    )?;
    ensure_write_matches(
        configuration.proxies().get(updated.id()),
        Some(&updated),
        ConfigurationWriteComponent::ProxyProfiles,
    )?;
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
