use std::str::FromStr;

use any2api_domain::{CredentialId, OAuthAccountId, RoutingCredentialId};
use axum::{
    Json,
    extract::{Path, State},
};

use crate::state::AppState;

use super::{
    dto::{AffinityClearResponse, AffinityRuntimeResponse},
    error::AdminApiError,
};

pub(crate) async fn get(State(state): State<AppState>) -> Json<AffinityRuntimeResponse> {
    let published = state.snapshots().load();
    let runtime = state
        .runtime()
        .affinity_snapshot(published.affinity_policy());
    Json(AffinityRuntimeResponse::new(&published, &runtime))
}

pub(crate) async fn clear_all(State(state): State<AppState>) -> Json<AffinityClearResponse> {
    Json(AffinityClearResponse::new(
        state.runtime().clear_all_affinity(),
    ))
}

pub(crate) async fn clear_credential(
    State(state): State<AppState>,
    Path(id): Path<String>,
) -> Result<Json<AffinityClearResponse>, AdminApiError> {
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
    Ok(Json(AffinityClearResponse::new(
        state.runtime().clear_routing_credential_affinity(id),
    )))
}
