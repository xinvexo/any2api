use std::str::FromStr;

use axum::{
    Json, Router,
    extract::{Path, State},
    routing::get,
};

use crate::{
    admin::{
        error::AdminApiError, request_json::AdminJson, revision::RequiredVersionedQuery,
        upstream_usage,
    },
    state::AppState,
};

use super::dto::{
    OAuthAccountCollectionResponse, OAuthAccountDeleteQuery, OAuthAccountModelsRequest,
    OAuthAccountUpdateRequest,
};
use super::mutation_response::OAuthAccountMutationResponse;

pub(in crate::admin::oauth) fn routes() -> Router<AppState> {
    Router::new()
        .route("/oauth/accounts", get(list))
        .route(
            "/oauth/accounts/{id}",
            axum::routing::patch(update).delete(delete),
        )
        .route(
            "/oauth/accounts/{id}/models",
            axum::routing::put(set_models),
        )
}

async fn list(State(state): State<AppState>) -> Json<OAuthAccountCollectionResponse> {
    accounts_response(&state, &state.snapshots().load()).await
}

async fn update(
    State(state): State<AppState>,
    Path(id): Path<String>,
    AdminJson(payload): AdminJson<OAuthAccountUpdateRequest>,
) -> Result<Json<OAuthAccountMutationResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, expected_config_version, draft, proxy_selection) = payload.into_domain()?;
    let snapshot = state
        .publisher()
        .update_oauth_account(
            expected,
            id,
            expected_config_version,
            draft,
            proxy_selection,
        )
        .await?;
    Ok(mutation_response(&snapshot, state.oauth()))
}

async fn set_models(
    State(state): State<AppState>,
    Path(id): Path<String>,
    AdminJson(payload): AdminJson<OAuthAccountModelsRequest>,
) -> Result<Json<OAuthAccountMutationResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, expected_config_version, models) = payload.into_domain()?;
    let snapshot = state
        .publisher()
        .set_oauth_account_models(expected, id, expected_config_version, models)
        .await?;
    Ok(mutation_response(&snapshot, state.oauth()))
}

async fn delete(
    State(state): State<AppState>,
    Path(id): Path<String>,
    RequiredVersionedQuery(query): RequiredVersionedQuery<OAuthAccountDeleteQuery>,
) -> Result<Json<OAuthAccountMutationResponse>, AdminApiError> {
    let id = parse_id(&id)?;
    let (expected, expected_config_version) = query.into_domain()?;
    let snapshot = state
        .publisher()
        .delete_oauth_account(expected, id, expected_config_version)
        .await?;
    Ok(mutation_response(&snapshot, state.oauth()))
}

async fn accounts_response(
    state: &AppState,
    snapshot: &any2api_runtime::api::PublishedSnapshot,
) -> Json<OAuthAccountCollectionResponse> {
    let usage = upstream_usage::load(state).await;
    let model_catalogs = match state.oauth() {
        Some(oauth) => match oauth.model_catalogs_for_accounts(snapshot).await {
            Ok(catalogs) => catalogs,
            Err(error) => {
                tracing::warn!(error = %error, "OAuth model catalog snapshots could not be loaded");
                Default::default()
            }
        },
        None => Default::default(),
    };
    Json(OAuthAccountCollectionResponse::from_snapshot(
        snapshot,
        &usage,
        state.oauth(),
        &model_catalogs,
    ))
}

fn mutation_response(
    snapshot: &any2api_runtime::api::PublishedSnapshot,
    oauth: Option<&any2api_runtime::api::OAuthService>,
) -> Json<OAuthAccountMutationResponse> {
    Json(OAuthAccountMutationResponse::from_snapshot(snapshot, oauth))
}

pub(in crate::admin::oauth) fn parse_id(
    value: &str,
) -> Result<any2api_domain::OAuthAccountId, AdminApiError> {
    any2api_domain::OAuthAccountId::from_str(value)
        .map_err(|_| AdminApiError::invalid_request("OAuth account id is invalid"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mutation_ack_is_snapshot_only_and_synchronous() {
        let _: fn(
            &any2api_runtime::api::PublishedSnapshot,
            Option<&any2api_runtime::api::OAuthService>,
        ) -> Json<OAuthAccountMutationResponse> = mutation_response;
    }
}
