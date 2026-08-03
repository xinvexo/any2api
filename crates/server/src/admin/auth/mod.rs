mod access;
mod cookie;
mod dto;
mod handlers;
mod middleware;

use super::error;
use crate::state::AppState;
use axum::{
    Router,
    routing::{get, post},
};

pub(super) use middleware::require_admin_session;

pub(super) fn public_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/session", get(handlers::session))
        .route("/auth/setup", post(handlers::setup))
        .route("/auth/login", post(handlers::login))
}

pub(super) fn protected_routes() -> Router<AppState> {
    Router::new()
        .route("/auth/logout", post(handlers::logout))
        .route("/auth/password/rotate", post(handlers::rotate_password))
}
