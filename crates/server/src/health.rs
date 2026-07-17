use axum::{Json, extract::State};
use serde::Serialize;

use crate::state::AppState;

#[derive(Debug, Serialize)]
pub(crate) struct HealthResponse {
    status: &'static str,
    config_revision: u64,
    scheduler_epoch: u64,
}

pub(crate) async fn health(State(state): State<AppState>) -> Json<HealthResponse> {
    let snapshot = state.snapshots().load();

    Json(HealthResponse {
        status: "ok",
        config_revision: snapshot.revision().get(),
        scheduler_epoch: state.runtime().scheduler_epoch(),
    })
}
