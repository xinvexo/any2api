use any2api_domain::ConfigRevision;
use sqlx::SqliteConnection;

use crate::{
    configuration::{
        StoredConfiguration, bump_revision, ensure_write_matches, load_configuration_from,
        readback_provider_endpoint_mutation,
    },
    error::{ConfigurationWriteComponent, StorageError},
    provider::{bump_endpoint_credential_generations, reconcile_model_routes},
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
        current.proxies(),
        mutation,
    )?
    else {
        return Ok((current, false));
    };
    let expected_model_routes = prepared.model_routes().cloned();
    let model_routes_changed = expected_model_routes.is_some();
    if prepared.deletes_endpoint() {
        reconcile_model_routes_and_allowlist(
            connection,
            current.model_routes(),
            expected_model_routes.as_ref(),
            current.settings(),
            current.oauth_accounts(),
        )
        .await?;
    }
    execute_provider_endpoint_change(connection, prepared.change()).await?;
    let credential_rows_changed = prepared.credential_rows_changed();
    let credential_generation_changed = prepared.bump_credential_generations();
    let credential_configuration_changed = credential_rows_changed || credential_generation_changed;
    if credential_generation_changed {
        bump_endpoint_credential_generations(connection, prepared.endpoint_id()).await?;
    }
    if !prepared.deletes_endpoint() {
        reconcile_model_routes_and_allowlist(
            connection,
            current.model_routes(),
            expected_model_routes.as_ref(),
            current.settings(),
            current.oauth_accounts(),
        )
        .await?;
    }
    let (expected_endpoints, expected_credentials) = prepared.into_configurations();
    let revision = bump_revision(connection, expected).await?;
    let configuration = readback_provider_endpoint_mutation(
        connection,
        current,
        credential_configuration_changed,
        model_routes_changed,
    )
    .await?;
    ensure_write_matches(
        configuration.revision(),
        revision,
        ConfigurationWriteComponent::Revision,
    )?;
    ensure_write_matches(
        configuration.provider_endpoints(),
        &expected_endpoints,
        ConfigurationWriteComponent::ProviderEndpoints,
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

async fn reconcile_model_routes_and_allowlist(
    connection: &mut SqliteConnection,
    current: &any2api_domain::ModelRouteConfiguration,
    candidate: Option<&any2api_domain::ModelRouteConfiguration>,
    settings: &any2api_domain::SettingsConfiguration,
    oauth_accounts: &any2api_domain::OAuthAccountConfiguration,
) -> Result<(), StorageError> {
    let Some(candidate) = candidate else {
        return Ok(());
    };
    reconcile_model_routes(connection, current, candidate).await?;
    prune_model_allowlist(connection, settings, candidate, oauth_accounts).await?;
    Ok(())
}
