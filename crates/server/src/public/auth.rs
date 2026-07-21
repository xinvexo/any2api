use std::sync::Arc;

use any2api_domain::GatewayApiKeyId;
use any2api_runtime::api::PublishedSnapshot;
use axum::{
    extract::{Request, State},
    http::{
        HeaderMap,
        header::{AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION},
    },
    middleware::Next,
    response::Response,
};

use crate::state::AppState;

use super::error::PublicApiError;

#[derive(Clone)]
pub(crate) struct AuthenticatedGatewayApiKey {
    id: GatewayApiKeyId,
    snapshot: Arc<PublishedSnapshot>,
}

impl AuthenticatedGatewayApiKey {
    pub(crate) const fn id(&self) -> GatewayApiKeyId {
        self.id
    }

    pub(crate) fn snapshot(&self) -> &PublishedSnapshot {
        &self.snapshot
    }

    pub(crate) fn snapshot_arc(&self) -> Arc<PublishedSnapshot> {
        Arc::clone(&self.snapshot)
    }
}

pub(crate) async fn require_gateway_api_key(
    State(state): State<AppState>,
    mut request: Request,
    next: Next,
) -> Response {
    let token = match extract_token(request.headers()) {
        Ok(token) => token,
        Err(error) => return error.into_response_for(&state, request.uri()),
    };
    let snapshot = state.snapshots().load();
    let Some(id) = snapshot.authenticate_gateway_api_key(&token) else {
        return PublicApiError::unauthorized().into_response_for(&state, request.uri());
    };

    strip_client_credentials(request.headers_mut());
    request
        .extensions_mut()
        .insert(AuthenticatedGatewayApiKey { id, snapshot });
    next.run(request).await
}

fn extract_token(headers: &HeaderMap) -> Result<String, PublicApiError> {
    let mut tokens = Vec::new();
    for value in headers.get_all(AUTHORIZATION).iter() {
        let text = value.to_str().map_err(|_| PublicApiError::unauthorized())?;
        let Some((scheme, token)) = text.split_once(' ') else {
            return Err(PublicApiError::unauthorized());
        };
        if !scheme.eq_ignore_ascii_case("bearer") || token.trim() != token || token.is_empty() {
            return Err(PublicApiError::unauthorized());
        }
        tokens.push(token.to_owned());
    }
    for value in headers.get_all("x-api-key").iter() {
        let token = value.to_str().map_err(|_| PublicApiError::unauthorized())?;
        if token.trim() != token || token.is_empty() {
            return Err(PublicApiError::unauthorized());
        }
        tokens.push(token.to_owned());
    }
    let Some(first) = tokens.first() else {
        return Err(PublicApiError::unauthorized());
    };
    if tokens.iter().any(|token| token != first) {
        return Err(PublicApiError::conflicting_credentials());
    }
    Ok(first.clone())
}

fn strip_client_credentials(headers: &mut HeaderMap) {
    headers.remove(AUTHORIZATION);
    headers.remove("x-api-key");
    headers.remove(PROXY_AUTHORIZATION);
    headers.remove(COOKIE);
    headers.remove("x-request-id");
}

#[cfg(test)]
mod tests {
    use axum::http::{
        HeaderMap, HeaderValue,
        header::{AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION},
    };

    use super::{extract_token, strip_client_credentials};

    #[test]
    fn equal_auth_headers_are_accepted_but_conflicts_are_rejected() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer same"));
        headers.insert("x-api-key", HeaderValue::from_static("same"));
        assert_eq!(extract_token(&headers).expect("same token"), "same");
        headers.insert("x-api-key", HeaderValue::from_static("different"));
        assert!(extract_token(&headers).is_err());
    }

    #[test]
    fn sensitive_headers_are_removed_before_downstream_handlers() {
        let mut headers = HeaderMap::new();
        headers.insert(AUTHORIZATION, HeaderValue::from_static("Bearer secret"));
        headers.insert("x-api-key", HeaderValue::from_static("secret"));
        headers.insert(PROXY_AUTHORIZATION, HeaderValue::from_static("proxy"));
        headers.insert(COOKIE, HeaderValue::from_static("session=secret"));
        strip_client_credentials(&mut headers);
        assert!(headers.get(AUTHORIZATION).is_none());
        assert!(headers.get("x-api-key").is_none());
        assert!(headers.get(PROXY_AUTHORIZATION).is_none());
        assert!(headers.get(COOKIE).is_none());
    }
}
