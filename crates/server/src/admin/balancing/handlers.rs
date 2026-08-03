use axum::{Json, extract::State};

use crate::state::AppState;

use super::dto::BalancingRuntimeResponse;

pub(crate) async fn get(State(state): State<AppState>) -> Json<BalancingRuntimeResponse> {
    let published = state.snapshots().load();
    let runtime = state.runtime().balancing_snapshot(&published);
    Json(BalancingRuntimeResponse::new(&published, &runtime))
}
