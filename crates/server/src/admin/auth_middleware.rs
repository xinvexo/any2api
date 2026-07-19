use std::net::SocketAddr;

use axum::{
    extract::{ConnectInfo, Request, State},
    http::Method,
    middleware::Next,
    response::{IntoResponse, Response},
};

use crate::state::AppState;

use super::{access, auth_cookie, error::AdminApiError, loopback};

pub(super) async fn require_admin_session(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(auth) = state.admin_auth() else {
        return loopback::require_loopback(request, next).await;
    };
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(address)| *address);
    let (connection, snapshot) = match access::resolve(&state, peer, request.headers()) {
        Ok(value) => value,
        Err(error) => return error.into_response(),
    };
    let token = match auth_cookie::read(request.headers()) {
        Ok(Some(token)) => token,
        Ok(None) | Err(_) => return AdminApiError::session_required().into_response(),
    };
    let Some(session) = auth.authenticate(token, snapshot.settings().admin()).await else {
        return AdminApiError::session_required().into_response();
    };
    if requires_csrf(request.method()) {
        let csrf = request
            .headers()
            .get("x-csrf-token")
            .and_then(|value| value.to_str().ok());
        if !csrf.is_some_and(|csrf| session.csrf_matches(csrf)) {
            return AdminApiError::csrf_invalid().into_response();
        }
    }
    request.extensions_mut().insert(connection);
    request.extensions_mut().insert(session);
    next.run(request).await
}

fn requires_csrf(method: &Method) -> bool {
    !matches!(*method, Method::GET | Method::HEAD | Method::OPTIONS)
}
