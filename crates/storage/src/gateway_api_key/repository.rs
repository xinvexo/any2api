use any2api_domain::{ConfigRevision, GatewayApiKeyVerifier};
use sqlx::SqliteConnection;

use crate::{
    configuration::{
        StoredConfiguration, bump_revision, ensure_write_matches, load_configuration_from,
        readback_gateway_api_key_mutation,
    },
    error::{ConfigurationWriteComponent, StorageError},
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
    let verifier = GatewayApiKeyVerifier::new();
    let Some(prepared) = prepare(current.gateway_api_keys(), &verifier, mutation)? else {
        return Ok((current, false));
    };
    execute_change(connection, prepared.change()).await?;
    let expected_keys = prepared.into_configuration();
    let revision = bump_revision(connection, expected).await?;
    let configuration = readback_gateway_api_key_mutation(connection, current).await?;
    ensure_write_matches(
        configuration.revision(),
        revision,
        ConfigurationWriteComponent::Revision,
    )?;
    ensure_write_matches(
        configuration.gateway_api_keys(),
        &expected_keys,
        ConfigurationWriteComponent::GatewayApiKeys,
    )?;
    Ok((configuration, true))
}
