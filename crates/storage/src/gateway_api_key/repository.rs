use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::{StoredConfiguration, bump_revision, load_configuration_from},
    error::StorageError,
    gateway_api_key::{
        mutation::{GatewayApiKeyMutation, prepare},
        writes::execute_change,
    },
};

pub(crate) async fn mutate_connection(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    mutation: GatewayApiKeyMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) = prepare(
        current.gateway_api_keys(),
        current.gateway_api_key_verifier(),
        mutation,
    )?
    else {
        return Ok((current, false));
    };
    execute_change(connection, prepared.change()).await?;
    let expected_keys = prepared.into_configuration();
    let revision = bump_revision(connection, expected).await?;
    drop(current);
    let configuration = load_configuration_from(connection).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.gateway_api_keys(), &expected_keys);
    Ok((configuration, true))
}
