use axum::{Json, extract::State};

use crate::state::AppState;

use super::dto::BalancingRuntimeResponse;

pub(crate) async fn get(State(state): State<AppState>) -> Json<BalancingRuntimeResponse> {
    let published = state.snapshots().load();
    let runtime = state.runtime().balancing_snapshot(&published);
    let lifecycle = state.runtime().lifecycle();
    let transport = state.public_requests().transport_runtime_snapshot();
    let telemetry = state.request_telemetry();
    Json(BalancingRuntimeResponse::new(
        &published,
        &runtime,
        &lifecycle,
        transport,
        telemetry.metrics(),
        telemetry.public_requests_in_window(),
    ))
}
