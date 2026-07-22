use axum::{extract::State, response::Response};

use crate::state::AppState;

use super::{balancing_dto::BalancingRuntimeResponse, no_store};

pub(crate) async fn get(State(state): State<AppState>) -> Response {
    let published = state.snapshots().load();
    let runtime = state.runtime().balancing_snapshot(&published);
    no_store::json(BalancingRuntimeResponse::new(&published, &runtime))
}
