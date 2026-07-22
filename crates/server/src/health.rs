use axum::{Json, extract::State};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    config_revision: u64,
    scheduler_epoch: u64,
    shutdown_phase: &'static str,
    active_requests: usize,
    background_tasks: usize,
}

pub(crate) async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let snapshot = state.snapshots().load();
    let lifecycle = state.runtime().lifecycle();

    Json(HealthResponse {
        status: "ok",
        config_revision: snapshot.revision().get(),
        scheduler_epoch: state.runtime().scheduler_epoch(),
        shutdown_phase: lifecycle.phase().as_str(),
        active_requests: lifecycle.active_requests().saturating_sub(1),
        background_tasks: lifecycle.background_task_count(),
    })
}
