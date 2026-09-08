use std::io;

use axum::{
    Json, Router,
    extract::{Query, State, rejection::QueryRejection},
    routing::get,
};

use crate::{
    runtime_log::{RuntimeLogPage, RuntimeLogQuery},
    state::AppState,
};

use super::AdminApiError;

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/runtime-logs", get(list))
}

async fn list(
    State(state): State<AppState>,
    query: Result<Query<RuntimeLogQuery>, QueryRejection>,
) -> Result<Json<RuntimeLogPage>, AdminApiError> {
    let query = query
        .map_err(|_| AdminApiError::invalid_request("runtime log query is invalid"))?
        .0;
    if query
        .cursor
        .as_ref()
        .is_some_and(|value| value.len() > 1024)
    {
        return Err(AdminApiError::invalid_request(
            "runtime log cursor is too long",
        ));
    }
    let source = state
        .runtime_logs()
        .ok_or_else(AdminApiError::system_log_unavailable)?;
    let mut page = source.list(query).await.map_err(|error| {
        if error.kind() == io::ErrorKind::InvalidInput {
            AdminApiError::invalid_request("runtime log cursor is invalid")
        } else {
            AdminApiError::system_log_unavailable()
        }
    })?;
    let snapshot = state.snapshots().load();
    for entry in &mut page.items {
        if let Some(id) = entry
            .fields
            .get("oauth_account_id")
            .and_then(|value| value.parse().ok())
            && let Some(account) = snapshot.oauth_accounts().get(id)
        {
            entry
                .fields
                .insert("oauth_account_name".into(), account.label().to_owned());
        }
    }
    Ok(Json(page))
}
