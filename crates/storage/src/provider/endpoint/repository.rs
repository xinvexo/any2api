use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::{StoredConfiguration, bump_revision, load_configuration_from},
    error::StorageError,
    provider::{bump_endpoint_credential_generations, replace_model_routes},
    settings::prune_model_allowlist,
};

use super::{
    mutation::{ProviderEndpointMutation, prepare_provider_endpoint_mutation},
    rows::execute_provider_endpoint_change,
};

pub(crate) async fn mutate_connection(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    mutation: ProviderEndpointMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) = prepare_provider_endpoint_mutation(
        current.provider_endpoints(),
        current.provider_credentials(),
        current.model_routes(),
        current.proxies(),
        mutation,
    )?
    else {
        return Ok((current, false));
    };
    execute_provider_endpoint_change(connection, prepared.change()).await?;
    if prepared.bump_credential_generations() {
        bump_endpoint_credential_generations(connection, prepared.endpoint_id()).await?;
    }
    let expected_model_routes = prepared.model_routes().cloned();
    if let Some(model_routes) = expected_model_routes.as_ref() {
        replace_model_routes(connection, model_routes).await?;
        prune_model_allowlist(
            connection,
            current.settings(),
            model_routes,
            current.oauth_accounts(),
        )
        .await?;
    }
    let (expected_endpoints, expected_credentials) = prepared.into_configurations();
    let revision = bump_revision(connection, expected).await?;
    let configuration = load_configuration_from(connection).await?;
    assert_eq!(configuration.revision(), revision);
    assert_eq!(configuration.provider_endpoints(), &expected_endpoints);
    assert_eq!(configuration.provider_credentials(), &expected_credentials);
    if let Some(expected_model_routes) = expected_model_routes {
        assert_eq!(configuration.model_routes(), &expected_model_routes);
    }
    Ok((configuration, true))
}
