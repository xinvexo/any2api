use std::str::FromStr;

use any2api_domain::{CredentialId, OAuthAccountId, RoutingCredentialId};
use axum::{
    extract::{Path, Query, State, rejection::QueryRejection},
    response::Response,
};

use crate::state::AppState;

use super::{
    affinity_dto::{AffinityClearResponse, AffinityQuery, AffinityRuntimeResponse},
    error::AdminApiError,
    no_store,
};

pub(crate) async fn get(
    State(state): State<AppState>,
    query: Result<Query<AffinityQuery>, QueryRejection>,
) -> Result<Response, AdminApiError> {
    let limit = query
        .map_err(|_| AdminApiError::invalid_request("affinity query is invalid"))?
        .0
        .limit()?;
    let published = state.snapshots().load();
    let runtime = state
        .runtime()
        .affinity_snapshot(published.affinity_policy(), limit);
    Ok(no_store::json(AffinityRuntimeResponse::new(
        &published, &runtime,
    )))
}

pub(crate) async fn clear_all(State(state): State<AppState>) -> Response {
    no_store::json(AffinityClearResponse::new(
        state.runtime().clear_all_affinity(),
    ))
}

pub(crate) async fn clear_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Response, AdminApiError> {
    let published = state.snapshots().load();
    let id = if let Some(id) = id.strip_prefix("oauth_account:") {
        let id = OAuthAccountId::from_str(id)
            .map_err(|_| AdminApiError::invalid_request("OAuth account id is invalid"))?;
        if published.oauth_accounts().get(id).is_none() {
            return Err(AdminApiError::oauth_account_not_found());
        }
        RoutingCredentialId::oauth_account(id)
    } else {
        let id = CredentialId::from_str(&id)
            .map_err(|_| AdminApiError::invalid_request("provider credential id is invalid"))?;
        if published.provider_credentials().get(id).is_none() {
            return Err(AdminApiError::provider_credential_not_found());
        }
        RoutingCredentialId::provider_credential(id)
    };
    Ok(no_store::json(AffinityClearResponse::new(
        state.runtime().clear_routing_credential_affinity(id),
    )))
}
