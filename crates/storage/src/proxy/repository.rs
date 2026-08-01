use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::{StoredConfiguration, bump_revision, load_configuration_from},
    error::StorageError,
};

use super::{
    mutation::{ProxyMutation, prepare_mutation},
    writes::execute_change,
};

pub(crate) async fn mutate_connection(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    mutation: ProxyMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) =
        prepare_mutation(current.proxies(), current.provider_credentials(), mutation)?
    else {
        return Ok((current, false));
    };
    execute_change(connection, prepared.change()).await?;
    let expected_proxies = prepared.into_configuration();
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.proxies(), &expected_proxies);
    Ok((configuration, true))
}
