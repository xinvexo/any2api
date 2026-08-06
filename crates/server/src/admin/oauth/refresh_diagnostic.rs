use any2api_runtime::api::{
    OAuthRefreshFailure, OAuthRefreshFailureReason, OAuthRefreshFailureScope,
    OAuthRefreshFailureStage, OAuthRefreshTrigger,
};
use serde::Serialize;

use crate::admin::error::AdminErrorDiagnostic;

#[derive(Clone, Debug, Serialize)]
pub(super) struct OAuthRefreshFailureResponse {
    token_version: u64,
    trigger: &'static str,
    stage: &'static str,
    reason: &'static str,
    upstream_status: Option<u16>,
    failure_scope: Option<&'static str>,
    occurred_at: i64,
    reauthorization_required: bool,
}

impl From<OAuthRefreshFailure> for OAuthRefreshFailureResponse {
    fn from(failure: OAuthRefreshFailure) -> Self {
        let fields = fields(failure);
        Self {
            token_version: failure.token_version(),
            trigger: fields.trigger,
            stage: fields.stage,
            reason: fields.reason,
            upstream_status: failure.upstream_status(),
            failure_scope: fields.failure_scope,
            occurred_at: failure.occurred_at(),
            reauthorization_required: failure.reauthorization_required(),
        }
    }
}

pub(super) fn error_diagnostic(failure: OAuthRefreshFailure) -> AdminErrorDiagnostic {
    let fields = fields(failure);
    AdminErrorDiagnostic::new(
        failure.token_version(),
        fields.trigger,
        fields.stage,
        fields.reason,
        failure.upstream_status(),
        fields.failure_scope,
        failure.occurred_at(),
        failure.reauthorization_required(),
    )
}

struct DiagnosticFields {
    trigger: &'static str,
    stage: &'static str,
    reason: &'static str,
    failure_scope: Option<&'static str>,
}

fn fields(failure: OAuthRefreshFailure) -> DiagnosticFields {
    DiagnosticFields {
        trigger: match failure.trigger() {
            OAuthRefreshTrigger::Scheduled => "scheduled",
            OAuthRefreshTrigger::AuthenticationFailure => "authentication_failure",
        },
        stage: match failure.stage() {
            OAuthRefreshFailureStage::Preflight => "preflight",
            OAuthRefreshFailureStage::RequestBuild => "request_build",
            OAuthRefreshFailureStage::Dns => "dns",
            OAuthRefreshFailureStage::Tcp => "tcp",
            OAuthRefreshFailureStage::ProxyHandshake => "proxy_handshake",
            OAuthRefreshFailureStage::Tls => "tls",
            OAuthRefreshFailureStage::WriteRequest => "write_request",
            OAuthRefreshFailureStage::AwaitHeaders => "await_headers",
            OAuthRefreshFailureStage::ReadResponse => "read_response",
            OAuthRefreshFailureStage::TokenEndpoint => "token_endpoint",
            OAuthRefreshFailureStage::ParseResponse => "parse_response",
            OAuthRefreshFailureStage::ValidateToken => "validate_token",
            OAuthRefreshFailureStage::PublishToken => "publish_token",
            OAuthRefreshFailureStage::VerifyAuthentication => "verify_authentication",
        },
        reason: match failure.reason() {
            OAuthRefreshFailureReason::AccountUnavailable => "account_unavailable",
            OAuthRefreshFailureReason::ProviderUnavailable => "provider_unavailable",
            OAuthRefreshFailureReason::TokenMaterialUnavailable => "token_material_unavailable",
            OAuthRefreshFailureReason::ProxyUnavailable => "proxy_unavailable",
            OAuthRefreshFailureReason::RefreshTokenMissing => "refresh_token_missing",
            OAuthRefreshFailureReason::RequestInvalid => "request_invalid",
            OAuthRefreshFailureReason::TransportFailure => "transport_failure",
            OAuthRefreshFailureReason::ReadTimeout => "read_timeout",
            OAuthRefreshFailureReason::ResponseTooLarge => "response_too_large",
            OAuthRefreshFailureReason::InvalidGrant => "invalid_grant",
            OAuthRefreshFailureReason::RefreshTokenExpired => "refresh_token_expired",
            OAuthRefreshFailureReason::RefreshTokenReused => "refresh_token_reused",
            OAuthRefreshFailureReason::RefreshTokenInvalidated => "refresh_token_invalidated",
            OAuthRefreshFailureReason::UpstreamRejected => "upstream_rejected",
            OAuthRefreshFailureReason::InvalidResponse => "invalid_response",
            OAuthRefreshFailureReason::ProviderMismatch => "provider_mismatch",
            OAuthRefreshFailureReason::RoutingProfileInvalid => "routing_profile_invalid",
            OAuthRefreshFailureReason::DocumentSerializationFailed => {
                "document_serialization_failed"
            }
            OAuthRefreshFailureReason::PublicationConflict => "publication_conflict",
            OAuthRefreshFailureReason::PublicationFailed => "publication_failed",
            OAuthRefreshFailureReason::RefreshUnavailable => "refresh_unavailable",
            OAuthRefreshFailureReason::RefreshedAccessTokenRejected => {
                "refreshed_access_token_rejected"
            }
        },
        failure_scope: failure.failure_scope().map(|scope| match scope {
            OAuthRefreshFailureScope::Endpoint => "endpoint",
            OAuthRefreshFailureScope::Proxy => "proxy",
            OAuthRefreshFailureScope::EgressPath => "egress_path",
            OAuthRefreshFailureScope::Unattributed => "unattributed",
        }),
    }
}
