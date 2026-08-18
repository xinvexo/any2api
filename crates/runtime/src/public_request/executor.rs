//! Public request execution and response lifecycle entry point.

use std::{
    net::IpAddr,
    pin::Pin,
    sync::{Arc, OnceLock},
};

use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError, RequestId};
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
    configuration::{GatewayApiKeyAuthProof, PublishedSnapshot, SnapshotStore},
    credential::RoutingPermit,
    oauth::{OAuthQuotaActivity, OAuthService, refresh::OAuthRefresher},
    request_telemetry::{RequestRecorder, RequestTelemetry, public_error_class},
    routing::{RouteCandidate, RouteInspectionSnapshot, inspect_routes},
};

use super::live_routing::LiveRoutingSnapshots;

#[derive(Clone)]
pub struct PublicRequest {
    pub request_id: RequestId,
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
        snapshots: Arc<SnapshotStore>,
        snapshot: Arc<PublishedSnapshot>,
        authentication: GatewayApiKeyAuthProof,
        request: PublicRequest,
    ) -> PublicResponse {
        let live = LiveRoutingSnapshots::new(snapshots, authentication);
        let policy = self
            .telemetry
            .policy(snapshot.revision(), snapshot.settings().logging());
        let recorder = RequestRecorder::new(
            Arc::clone(&self.telemetry),
            policy,
            request.request_id,
            authentication.id(),
            request.client_ip,
            request.operation,
        );
        let adapter = Arc::clone(
            self.protocols
                .get(request.operation.dialect())
                .expect("validated protocol registry"),
        );
        let result = self
            .execute_inner(
                snapshot,
                request,
                Arc::clone(&adapter),
                recorder.clone(),
                live,
            )
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
        live: LiveRoutingSnapshots,
    ) -> Result<PublicResponse, FinalFailure> {
        live.validate(snapshot.as_ref())
            .map_err(FinalFailure::from)?;
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
        let replan_mode = planning::RoutingReplanMode::for_request(snapshot.as_ref(), &planned);
        retry::execute(retry::RetryExecutionInput {
            policy_snapshot: Arc::clone(&snapshot),
            routing_snapshot: snapshot,
            protocols: Arc::clone(&self.protocols),
            plan: planned,
            replan_mode,
            providers: self.providers.as_ref(),
            transport: self.transport.as_ref(),
            live,
            oauth: self.oauth.get().map(|oauth| {
                retry::OAuthRetryServices::new(oauth.refresher.as_ref(), &oauth.quota_activity)
            }),
            recorder,
        })
        .await
    }
}

pub(super) struct SelectedCandidate {
    pub(super) candidate: RouteCandidate,
    pub(super) permit: RoutingPermit,
    pub(super) health: crate::health::AttemptHealth,
}

impl SelectedCandidate {
    pub(super) fn try_start_attempt(self) -> Result<Self, ()> {
        let Self {
            candidate,
            permit,
            health,
        } = self;
        match permit.try_start_attempt() {
            Ok(permit) => Ok(Self {
                candidate,
                permit,
                health,
            }),
            Err(_) => {
                drop(health);
                Err(())
            }
        }
    }

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
