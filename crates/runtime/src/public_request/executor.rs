//! Public request execution and response lifecycle entry point.

use std::{
    net::IpAddr,
    pin::Pin,
    sync::{Arc, OnceLock},
};

use any2api_domain::{GatewayApiKeyId, ProtocolDialect, ProtocolOperation, PublicError, RequestId};
use any2api_protocol::api::{EgressResponse, ProtocolAdapter, ProtocolRegistry};
use any2api_provider::api::ProviderRegistry;
use any2api_transport::api::{TransportManager, TransportRuntimeSnapshot};
use bytes::Bytes;
use futures_util::Stream;
use http::{HeaderMap, StatusCode};
use thiserror::Error;

use super::response::{
    FinalFailure, sanitize_response_headers, sanitize_upstream_error_response_headers,
};
use super::{planning, retry};
use crate::{
    configuration::PublishedSnapshot,
    credential::RoutingPermit,
    oauth::{OAuthQuotaActivity, OAuthService, refresh::OAuthRefresher},
    request_telemetry::{RequestRecorder, RequestTelemetry, public_error_class},
    routing::{RouteCandidate, RouteInspectionSnapshot, inspect_routes},
};

#[derive(Clone)]
pub struct PublicRequest {
    pub request_id: RequestId,
    pub gateway_api_key_id: GatewayApiKeyId,
    pub client_ip: IpAddr,
    pub operation: ProtocolOperation,
    pub headers: HeaderMap,
    pub body: Bytes,
}

pub struct PublicResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: PublicResponseBody,
}

pub type PublicResponseStream =
    Pin<Box<dyn Stream<Item = Result<Bytes, std::io::Error>> + Send + 'static>>;

pub enum PublicResponseBody {
    Buffered(Bytes),
    Streaming(PublicResponseStream),
}

pub struct PublicRequestService {
    protocols: Arc<ProtocolRegistry>,
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
    telemetry: Arc<RequestTelemetry>,
    oauth: OnceLock<OAuthRequestServices>,
}

struct OAuthRequestServices {
    refresher: Arc<OAuthRefresher>,
    quota_activity: OAuthQuotaActivity,
}

impl PublicRequestService {
    pub fn new(
        protocols: Arc<ProtocolRegistry>,
        providers: Arc<ProviderRegistry>,
        transport: Arc<dyn TransportManager>,
    ) -> Result<Self, PublicRequestServiceError> {
        for dialect in ProtocolOperation::ALL.map(ProtocolOperation::dialect) {
            if protocols.get(dialect).is_none() {
                return Err(PublicRequestServiceError::MissingProtocol(dialect));
            }
        }
        Ok(Self {
            protocols,
            providers,
            transport,
            telemetry: Arc::new(RequestTelemetry::disabled()),
            oauth: OnceLock::new(),
        })
    }

    #[must_use]
    pub fn with_telemetry(mut self, telemetry: Arc<RequestTelemetry>) -> Self {
        self.telemetry = telemetry;
        self
    }

    pub fn install_oauth(&self, oauth: &OAuthService) -> bool {
        self.oauth
            .set(OAuthRequestServices {
                refresher: oauth.refresher(),
                quota_activity: oauth.quota_activity(),
            })
            .is_ok()
    }

    pub async fn execute(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        request: PublicRequest,
    ) -> PublicResponse {
        let policy = self
            .telemetry
            .policy(snapshot.revision(), snapshot.settings().logging());
        let recorder = RequestRecorder::new(
            Arc::clone(&self.telemetry),
            policy,
            request.request_id,
            request.gateway_api_key_id,
            request.client_ip,
            request.operation,
        );
        let adapter = Arc::clone(
            self.protocols
                .get(request.operation.dialect())
                .expect("validated protocol registry"),
        );
        let result = self
            .execute_inner(snapshot, request, Arc::clone(&adapter), recorder.clone())
            .await;
        match result {
            Ok(response) => {
                if matches!(response.body, PublicResponseBody::Buffered(_)) {
                    recorder.finish(response.status.as_u16(), None);
                }
                response
            }
            Err(FinalFailure::Local { error }) => {
                let mut response = adapter.error_response(&error);
                sanitize_response_headers(&mut response.headers);
                recorder.finish_with_message(
                    response.status.as_u16(),
                    Some(public_error_class(error.code())),
                    Some(error.telemetry_message().to_owned()),
                );
                response.into()
            }
            Err(FinalFailure::Upstream {
                mut response,
                error_class,
                error_message,
            }) => {
                sanitize_upstream_error_response_headers(&mut response.headers, &response.body);
                recorder.finish_with_message(
                    response.status.as_u16(),
                    Some(error_class),
                    error_message,
                );
                response.into()
            }
        }
    }

    #[must_use]
    pub fn error_response(&self, dialect: ProtocolDialect, error: &PublicError) -> PublicResponse {
        self.protocols
            .get(dialect)
            .expect("validated protocol registry")
            .error_response(error)
            .into()
    }

    #[must_use]
    pub fn route_inspection(&self, snapshot: &PublishedSnapshot) -> RouteInspectionSnapshot {
        inspect_routes(snapshot, self.protocols.as_ref(), self.providers.as_ref())
    }

    #[must_use]
    pub fn transport_runtime_snapshot(&self) -> Option<TransportRuntimeSnapshot> {
        self.transport.runtime_snapshot()
    }

    async fn execute_inner(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        request: PublicRequest,
        adapter: Arc<dyn ProtocolAdapter>,
        recorder: RequestRecorder,
    ) -> Result<PublicResponse, FinalFailure> {
        let decoded = planning::decode(request, adapter.as_ref())
            .await
            .map_err(FinalFailure::from)?;
        recorder.set_request_metadata(
            decoded.public_model.as_str().to_owned(),
            decoded.decoded.stream,
            decoded.decoded.thinking_level.clone(),
        );
        let planned = planning::plan(
            snapshot.as_ref(),
            decoded,
            self.protocols.as_ref(),
            self.providers.as_ref(),
        )
        .map_err(FinalFailure::from)?;
        retry::execute(
            snapshot,
            Arc::clone(&self.protocols),
            planned,
            self.providers.as_ref(),
            self.transport.as_ref(),
            self.oauth.get().map(|oauth| {
                retry::OAuthRetryServices::new(oauth.refresher.as_ref(), &oauth.quota_activity)
            }),
            recorder,
        )
        .await
    }
}

pub(super) struct SelectedCandidate {
    pub(super) candidate: RouteCandidate,
    pub(super) permit: RoutingPermit,
    pub(super) health: crate::health::AttemptHealth,
}

impl SelectedCandidate {
    pub(super) fn rollback_before_attempt(self) {
        let Self { permit, health, .. } = self;
        drop(health);
        permit.rollback_before_attempt();
    }
}

pub(super) type RequestPermit = RoutingPermit;

impl From<EgressResponse> for PublicResponse {
    fn from(response: EgressResponse) -> Self {
        Self {
            status: response.status,
            headers: response.headers,
            body: PublicResponseBody::Buffered(response.body),
        }
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PublicRequestServiceError {
    #[error("missing protocol adapter for {0:?}")]
    MissingProtocol(ProtocolDialect),
}
