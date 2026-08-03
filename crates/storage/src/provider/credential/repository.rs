use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::{
        StoredConfiguration, bump_revision, ensure_write_matches, load_configuration_from,
        readback_provider_credential_mutation,
    },
    error::{ConfigurationWriteComponent, StorageError},
    provider::reconcile_model_routes,
    settings::prune_model_allowlist,
};

use super::{
    mutation::{ProviderCredentialMutation, prepare_provider_credential_mutation},
    writes::execute_provider_credential_change,
};

pub(crate) async fn mutate_connection(
    connection: &mut SqliteConnection,
    expected: ConfigRevision,
    mutation: ProviderCredentialMutation,
) -> Result<(StoredConfiguration, bool), StorageError> {
    let current = load_configuration_from(connection).await?;
    if current.revision() != expected {
        return Err(StorageError::RevisionConflict {
            expected,
            actual: current.revision(),
        });
    }
    let Some(prepared) = prepare_provider_credential_mutation(
        current.provider_credentials(),
        current.provider_endpoints(),
        current.proxies(),
        mutation,
    )?
    else {
        return Ok((current, false));
    };
    execute_provider_credential_change(connection, prepared.change()).await?;
    let expected_model_routes = prepared.model_routes().cloned();
    let model_routes_changed = expected_model_routes.is_some();
    if let Some(model_routes) = expected_model_routes.as_ref() {
        reconcile_model_routes(connection, current.model_routes(), model_routes).await?;
        prune_model_allowlist(
            connection,
            current.settings(),
            model_routes,
            current.oauth_accounts(),
        )
        .await?;
    }
    let expected_credentials = prepared.into_configuration();
    let revision = bump_revision(connection, expected).await?;
    let configuration =
        readback_provider_credential_mutation(connection, current, model_routes_changed).await?;
    ensure_write_matches(
        configuration.revision(),
        revision,
        ConfigurationWriteComponent::Revision,
    )?;
    ensure_write_matches(
        configuration.provider_credentials(),
        &expected_credentials,
        ConfigurationWriteComponent::ProviderCredentials,
    )?;
    if let Some(expected_model_routes) = expected_model_routes {
        ensure_write_matches(
            configuration.model_routes(),
            &expected_model_routes,
            ConfigurationWriteComponent::ModelRoutes,
        )?;
    }
    Ok((configuration, true))
}
