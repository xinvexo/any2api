use axum::{Json, extract::State};

use crate::state::AppState;

use super::dto::RouteInspectionResponse;

pub(crate) async fn get(State(state): State<AppState>) -> Json<RouteInspectionResponse> {
    let published = state.snapshots().load();
    Json(state.public_requests().route_inspection(&published).into())
}
