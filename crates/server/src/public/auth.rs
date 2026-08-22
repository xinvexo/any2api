use std::sync::Arc;

use any2api_runtime::api::{GatewayApiKeyAuthProof, PublishedSnapshot};
use axum::{
    extract::{Request, State},
    http::{
        HeaderMap, Uri,
        header::{AUTHORIZATION, COOKIE, PROXY_AUTHORIZATION},
    },
    middleware::Next,
    response::Response,
};

use crate::{
    client_address::ClientAddressContext, http_access_log::GatewayAuthRejected, state::AppState,
};

use super::error::PublicApiError;

#[derive(Clone)]
pub(crate) struct AuthenticatedGatewayApiKey {
    authentication: GatewayApiKeyAuthProof,
    client_ip: std::net::IpAddr,
    snapshot: Arc<PublishedSnapshot>,
}

impl AuthenticatedGatewayApiKey {
    pub(crate) const fn client_ip(&self) -> std::net::IpAddr {
        self.client_ip
    }

    pub(crate) const fn authentication(&self) -> GatewayApiKeyAuthProof {
        self.authentication
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
    let Some(client_context) = request.extensions().get::<ClientAddressContext>().cloned() else {
        tracing::error!("public request client-address context is missing");
        return reject(
            PublicApiError::invalid_forwarded_headers(),
            &state,
            request.uri(),
        );
    };
    let snapshot = client_context.snapshot_arc();
    let connection = match client_context.connection() {
        Ok(connection) => connection,
        Err(_) => {
            return reject(
                PublicApiError::invalid_forwarded_headers(),
                &state,
                request.uri(),
            );
        }
    };
    let token = match extract_token(request.headers()) {
        Ok(token) => token,
        Err(error) => return reject(error, &state, request.uri()),
    };
    let Some(authentication) = snapshot.authenticate_gateway_api_key(&token) else {
        return reject(PublicApiError::unauthorized(), &state, request.uri());
    };
    let telemetry = state.request_telemetry();
    telemetry.record_public_request();
    telemetry.record_gateway_key_use(authentication.id(), snapshot.revision());

    strip_client_credentials(request.headers_mut());
    request.extensions_mut().insert(AuthenticatedGatewayApiKey {
        authentication,
        client_ip: connection.client_ip(),
        snapshot,
    });
    next.run(request).await
}

fn reject(error: PublicApiError, state: &AppState, uri: &Uri) -> Response {
    let mut response = error.into_response_for(state, uri);
    response.extensions_mut().insert(GatewayAuthRejected);
    response
}

fn extract_token(headers: &HeaderMap) -> Result<String, PublicApiError> {
    let mut first: Option<&str> = None;
    let mut conflicting = false;
    for value in headers.get_all(AUTHORIZATION).iter() {
        let text = value.to_str().map_err(|_| PublicApiError::unauthorized())?;
        let Some((scheme, token)) = text.split_once(' ') else {
            return Err(PublicApiError::unauthorized());
        };
        if !scheme.eq_ignore_ascii_case("bearer") || token.trim() != token || token.is_empty() {
            return Err(PublicApiError::unauthorized());
        }
        match first {
            None => first = Some(token),
            Some(existing) => conflicting |= existing != token,
        }
    }
    for value in headers.get_all("x-api-key").iter() {
        let token = value.to_str().map_err(|_| PublicApiError::unauthorized())?;
        if token.trim() != token || token.is_empty() {
            return Err(PublicApiError::unauthorized());
        }
        match first {
            None => first = Some(token),
            Some(existing) => conflicting |= existing != token,
        }
    }
    let Some(first) = first else {
        return Err(PublicApiError::unauthorized());
    };
    if conflicting {
        return Err(PublicApiError::conflicting_credentials());
    }
    Ok(first.to_owned())
}

fn strip_client_credentials(headers: &mut HeaderMap) {
    headers.remove(AUTHORIZATION);
    headers.remove("x-api-key");
    headers.remove(PROXY_AUTHORIZATION);
    headers.remove(COOKIE);
    headers.remove("x-request-id");
    for name in [
        "chatgpt-account-id",
        "openai-organization",
        "openai-project",
        "x-openai-api-key",
        "x-userid",
        "x-xai-token-auth",
        "x-authenticateresponse",
    ] {
        headers.remove(name);
    }
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
        headers.insert("chatgpt-account-id", HeaderValue::from_static("account"));
        headers.insert("x-userid", HeaderValue::from_static("subject"));
        headers.insert("x-xai-token-auth", HeaderValue::from_static("spoofed"));
        strip_client_credentials(&mut headers);
        assert!(headers.get(AUTHORIZATION).is_none());
        assert!(headers.get("x-api-key").is_none());
        assert!(headers.get(PROXY_AUTHORIZATION).is_none());
        assert!(headers.get(COOKIE).is_none());
        assert!(headers.get("chatgpt-account-id").is_none());
        assert!(headers.get("x-userid").is_none());
        assert!(headers.get("x-xai-token-auth").is_none());
    }
}
