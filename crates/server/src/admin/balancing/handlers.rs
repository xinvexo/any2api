use axum::{Json, extract::State};

use crate::state::AppState;

use super::dto::BalancingRuntimeResponse;

pub(crate) async fn get(State(state): State<AppState>) -> Json<BalancingRuntimeResponse> {
    let published = state.snapshots().load();
    let runtime = state.runtime().balancing_snapshot(&published);
    let lifecycle = state.runtime().lifecycle();
    Json(BalancingRuntimeResponse::new(
        &published,
        &runtime,
        lifecycle.active_requests().saturating_sub(1),
        lifecycle.background_task_count(),
    ))
}
