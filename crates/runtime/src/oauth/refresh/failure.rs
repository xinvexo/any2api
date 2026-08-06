use any2api_provider::api::{OAuthRefreshRejection, ProviderError};
use any2api_transport::api::{TransportErrorStage, TransportFailureScope as TransportScope};
use thiserror::Error;

use crate::{
    configuration::ConfigPublishError,
    oauth::{document, error::OAuthError},
};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthRefreshTrigger {
    Scheduled,
    AuthenticationFailure,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthRefreshFailureStage {
    Preflight,
    RequestBuild,
    Dns,
    Tcp,
    ProxyHandshake,
    Tls,
    WriteRequest,
    AwaitHeaders,
    ReadResponse,
    TokenEndpoint,
    ParseResponse,
    ValidateToken,
    PublishToken,
    VerifyAuthentication,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthRefreshFailureReason {
    AccountUnavailable,
    ProviderUnavailable,
    TokenMaterialUnavailable,
    ProxyUnavailable,
    RefreshTokenMissing,
    RequestInvalid,
    TransportFailure,
    ReadTimeout,
    ResponseTooLarge,
    InvalidGrant,
    RefreshTokenExpired,
    RefreshTokenReused,
    RefreshTokenInvalidated,
    UpstreamRejected,
    InvalidResponse,
    ProviderMismatch,
    RoutingProfileInvalid,
    DocumentSerializationFailed,
    PublicationConflict,
    PublicationFailed,
    RefreshUnavailable,
    RefreshedAccessTokenRejected,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthRefreshFailureScope {
    Endpoint,
    Proxy,
    EgressPath,
    Unattributed,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OAuthRefreshFailure {
    token_version: u64,
    trigger: OAuthRefreshTrigger,
    stage: OAuthRefreshFailureStage,
    reason: OAuthRefreshFailureReason,
    upstream_status: Option<u16>,
    failure_scope: Option<OAuthRefreshFailureScope>,
    occurred_at: i64,
}

impl OAuthRefreshFailure {
    pub(super) fn new(
        token_version: u64,
        trigger: OAuthRefreshTrigger,
        stage: OAuthRefreshFailureStage,
        reason: OAuthRefreshFailureReason,
        upstream_status: Option<u16>,
        failure_scope: Option<OAuthRefreshFailureScope>,
    ) -> Self {
        Self {
            token_version,
            trigger,
            stage,
            reason,
            upstream_status,
            failure_scope,
            occurred_at: document::unix_now(),
        }
    }

    #[must_use]
    pub const fn token_version(self) -> u64 {
        self.token_version
    }

    #[must_use]
    pub const fn trigger(self) -> OAuthRefreshTrigger {
        self.trigger
    }

    #[must_use]
    pub const fn stage(self) -> OAuthRefreshFailureStage {
        self.stage
    }

    #[must_use]
    pub const fn reason(self) -> OAuthRefreshFailureReason {
        self.reason
    }

    #[must_use]
    pub const fn upstream_status(self) -> Option<u16> {
        self.upstream_status
    }

    #[must_use]
    pub const fn failure_scope(self) -> Option<OAuthRefreshFailureScope> {
        self.failure_scope
    }

    #[must_use]
    pub const fn occurred_at(self) -> i64 {
        self.occurred_at
    }

    #[must_use]
    pub const fn reauthorization_required(self) -> bool {
        matches!(
            self.reason,
            OAuthRefreshFailureReason::RefreshTokenMissing
                | OAuthRefreshFailureReason::InvalidGrant
                | OAuthRefreshFailureReason::RefreshTokenExpired
                | OAuthRefreshFailureReason::RefreshTokenReused
                | OAuthRefreshFailureReason::RefreshTokenInvalidated
                | OAuthRefreshFailureReason::RefreshedAccessTokenRejected
        )
    }

    pub(super) fn missing_refresh_token(token_version: u64, trigger: OAuthRefreshTrigger) -> Self {
        Self::new(
            token_version,
            trigger,
            OAuthRefreshFailureStage::Preflight,
            OAuthRefreshFailureReason::RefreshTokenMissing,
            None,
            None,
        )
    }

    pub(super) fn refresh_unavailable(token_version: u64, trigger: OAuthRefreshTrigger) -> Self {
        Self::new(
            token_version,
            trigger,
            OAuthRefreshFailureStage::Preflight,
            OAuthRefreshFailureReason::RefreshUnavailable,
            None,
            None,
        )
    }

    pub(super) fn refreshed_access_token_rejected(token_version: u64) -> Self {
        Self::new(
            token_version,
            OAuthRefreshTrigger::AuthenticationFailure,
            OAuthRefreshFailureStage::VerifyAuthentication,
            OAuthRefreshFailureReason::RefreshedAccessTokenRejected,
            Some(http::StatusCode::UNAUTHORIZED.as_u16()),
            None,
        )
    }

    pub(super) fn permanent_rejection(
        token_version: u64,
        trigger: OAuthRefreshTrigger,
        rejection: OAuthRefreshRejection,
        upstream_status: Option<u16>,
    ) -> Self {
        Self::new(
            token_version,
            trigger,
            OAuthRefreshFailureStage::TokenEndpoint,
            rejection_reason(rejection),
            upstream_status,
            None,
        )
    }
}

#[derive(Debug, Error)]
pub(super) enum OAuthRefreshError {
    #[error("OAuth account disappeared after refresh publication")]
    AccountUnavailable,
    #[error("OAuth provider driver is unavailable")]
    ProviderUnavailable,
    #[error("OAuth token material is unavailable")]
    TokenMaterialUnavailable,
    #[error("OAuth refresh request could not be constructed")]
    RequestBuild(#[source] ProviderError),
    #[error("OAuth refresh endpoint rejected the token")]
    RefreshRejected {
        status: u16,
        rejection: OAuthRefreshRejection,
    },
    #[error("OAuth refresh request failed")]
    OAuth(#[source] OAuthError),
    #[error("OAuth refresh response is invalid")]
    TokenResponseInvalid,
    #[error("OAuth refresh token validation failed")]
    TokenValidation(#[source] ProviderError),
    #[error("OAuth refresh response provider did not match the account")]
    ProviderMismatch,
    #[error("OAuth routing profile validation failed")]
    RoutingProfile(#[source] ProviderError),
    #[error("OAuth authentication document could not be generated")]
    DocumentSerialization,
    #[error("OAuth refresh publication failed")]
    Publish(#[source] ConfigPublishError),
}

impl OAuthRefreshError {
    pub(super) fn failure(
        &self,
        token_version: u64,
        trigger: OAuthRefreshTrigger,
    ) -> OAuthRefreshFailure {
        let (stage, reason, status, scope) = match self {
            Self::AccountUnavailable => (
                OAuthRefreshFailureStage::PublishToken,
                OAuthRefreshFailureReason::AccountUnavailable,
                None,
                None,
            ),
            Self::ProviderUnavailable => (
                OAuthRefreshFailureStage::Preflight,
                OAuthRefreshFailureReason::ProviderUnavailable,
                None,
                None,
            ),
            Self::TokenMaterialUnavailable => (
                OAuthRefreshFailureStage::Preflight,
                OAuthRefreshFailureReason::TokenMaterialUnavailable,
                None,
                None,
            ),
            Self::RequestBuild(_) => (
                OAuthRefreshFailureStage::RequestBuild,
                OAuthRefreshFailureReason::RequestInvalid,
                None,
                None,
            ),
            Self::RefreshRejected { status, rejection } => (
                OAuthRefreshFailureStage::TokenEndpoint,
                rejection_reason(*rejection),
                Some(*status),
                None,
            ),
            Self::OAuth(error) => oauth_error_fields(error),
            Self::TokenResponseInvalid => (
                OAuthRefreshFailureStage::ParseResponse,
                OAuthRefreshFailureReason::InvalidResponse,
                None,
                None,
            ),
            Self::TokenValidation(_) => (
                OAuthRefreshFailureStage::ValidateToken,
                OAuthRefreshFailureReason::InvalidResponse,
                None,
                None,
            ),
            Self::ProviderMismatch => (
                OAuthRefreshFailureStage::ValidateToken,
                OAuthRefreshFailureReason::ProviderMismatch,
                None,
                None,
            ),
            Self::RoutingProfile(_) => (
                OAuthRefreshFailureStage::ValidateToken,
                OAuthRefreshFailureReason::RoutingProfileInvalid,
                None,
                None,
            ),
            Self::DocumentSerialization => (
                OAuthRefreshFailureStage::ValidateToken,
                OAuthRefreshFailureReason::DocumentSerializationFailed,
                None,
                None,
            ),
            Self::Publish(error) => (
                OAuthRefreshFailureStage::PublishToken,
                publish_reason(error),
                None,
                None,
            ),
        };
        OAuthRefreshFailure::new(token_version, trigger, stage, reason, status, scope)
    }
}

impl From<OAuthError> for OAuthRefreshError {
    fn from(error: OAuthError) -> Self {
        Self::OAuth(error)
    }
}

fn rejection_reason(rejection: OAuthRefreshRejection) -> OAuthRefreshFailureReason {
    match rejection {
        OAuthRefreshRejection::InvalidGrant => OAuthRefreshFailureReason::InvalidGrant,
        OAuthRefreshRejection::RefreshTokenExpired => {
            OAuthRefreshFailureReason::RefreshTokenExpired
        }
        OAuthRefreshRejection::RefreshTokenReused => OAuthRefreshFailureReason::RefreshTokenReused,
        OAuthRefreshRejection::RefreshTokenInvalidated => {
            OAuthRefreshFailureReason::RefreshTokenInvalidated
        }
        OAuthRefreshRejection::Unverified => OAuthRefreshFailureReason::UpstreamRejected,
    }
}

fn oauth_error_fields(
    error: &OAuthError,
) -> (
    OAuthRefreshFailureStage,
    OAuthRefreshFailureReason,
    Option<u16>,
    Option<OAuthRefreshFailureScope>,
) {
    match error {
        OAuthError::Transport(error) => (
            transport_stage(error.stage),
            OAuthRefreshFailureReason::TransportFailure,
            None,
            Some(transport_scope(error.failure_scope)),
        ),
        OAuthError::TokenRejected(status) => (
            OAuthRefreshFailureStage::TokenEndpoint,
            OAuthRefreshFailureReason::UpstreamRejected,
            Some(*status),
            None,
        ),
        OAuthError::TokenReadTimeout => (
            OAuthRefreshFailureStage::ReadResponse,
            OAuthRefreshFailureReason::ReadTimeout,
            None,
            None,
        ),
        OAuthError::TokenResponseTooLarge => (
            OAuthRefreshFailureStage::ReadResponse,
            OAuthRefreshFailureReason::ResponseTooLarge,
            None,
            None,
        ),
        OAuthError::TokenResponseInvalid => (
            OAuthRefreshFailureStage::ParseResponse,
            OAuthRefreshFailureReason::InvalidResponse,
            None,
            None,
        ),
        OAuthError::PublishedProxyUnavailable => (
            OAuthRefreshFailureStage::Preflight,
            OAuthRefreshFailureReason::ProxyUnavailable,
            None,
            Some(OAuthRefreshFailureScope::Proxy),
        ),
        OAuthError::DocumentSerialization => (
            OAuthRefreshFailureStage::ValidateToken,
            OAuthRefreshFailureReason::DocumentSerializationFailed,
            None,
            None,
        ),
        OAuthError::Provider(_) => (
            OAuthRefreshFailureStage::RequestBuild,
            OAuthRefreshFailureReason::RequestInvalid,
            None,
            None,
        ),
        _ => (
            OAuthRefreshFailureStage::Preflight,
            OAuthRefreshFailureReason::RefreshUnavailable,
            None,
            None,
        ),
    }
}

fn transport_stage(stage: TransportErrorStage) -> OAuthRefreshFailureStage {
    match stage {
        TransportErrorStage::Dns => OAuthRefreshFailureStage::Dns,
        TransportErrorStage::Tcp => OAuthRefreshFailureStage::Tcp,
        TransportErrorStage::ProxyHandshake => OAuthRefreshFailureStage::ProxyHandshake,
        TransportErrorStage::Tls => OAuthRefreshFailureStage::Tls,
        TransportErrorStage::WriteRequest => OAuthRefreshFailureStage::WriteRequest,
        TransportErrorStage::AwaitHeaders => OAuthRefreshFailureStage::AwaitHeaders,
        TransportErrorStage::ReadBody => OAuthRefreshFailureStage::ReadResponse,
    }
}

fn transport_scope(scope: TransportScope) -> OAuthRefreshFailureScope {
    match scope {
        TransportScope::Endpoint => OAuthRefreshFailureScope::Endpoint,
        TransportScope::Proxy => OAuthRefreshFailureScope::Proxy,
        TransportScope::EgressPath => OAuthRefreshFailureScope::EgressPath,
        TransportScope::Unattributed => OAuthRefreshFailureScope::Unattributed,
    }
}

fn publish_reason(error: &ConfigPublishError) -> OAuthRefreshFailureReason {
    match error {
        ConfigPublishError::OAuthAccountNotFound => OAuthRefreshFailureReason::AccountUnavailable,
        ConfigPublishError::RevisionConflict { .. }
        | ConfigPublishError::OAuthAccountTokenVersionConflict => {
            OAuthRefreshFailureReason::PublicationConflict
        }
        _ => OAuthRefreshFailureReason::PublicationFailed,
    }
}
