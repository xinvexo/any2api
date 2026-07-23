mod affinity;
mod planning;
mod response;
mod retry;
mod selection;
mod stream;
mod upstream;

#[cfg(test)]
mod stream_tests;
#[cfg(test)]
mod stream_timeout_tests;

use std::{pin::Pin, sync::Arc};

use any2api_domain::{GatewayApiKeyId, ProtocolDialect, ProtocolOperation, PublicError, RequestId};
use any2api_protocol::api::{EgressResponse, ProtocolAdapter, ProtocolRegistry};
use any2api_provider::api::{CredentialHeaders, ProviderDriver, ProviderError, ProviderRegistry};
use any2api_transport::api::TransportManager;
use bytes::Bytes;
use futures_util::Stream;
use http::{HeaderMap, StatusCode};
use thiserror::Error;

use crate::{
    auxiliary_scheduler::AuxiliaryPermit,
    credential_runtime::ConcurrencyPermit,
    published_snapshot::PublishedSnapshot,
    request_telemetry::{RequestRecorder, RequestTelemetry},
    route_candidates::RouteCandidate,
};

#[derive(Clone)]
pub struct PublicRequest {
    pub request_id: RequestId,
    pub gateway_api_key_id: GatewayApiKeyId,
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
        })
    }

    #[must_use]
    pub fn with_telemetry(mut self, telemetry: Arc<RequestTelemetry>) -> Self {
        self.telemetry = telemetry;
        self
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
            Err(error) => {
                let response = adapter.error_response(&error);
                recorder.finish_public_error(response.status.as_u16(), &error);
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

    async fn execute_inner(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        request: PublicRequest,
        adapter: Arc<dyn ProtocolAdapter>,
        recorder: RequestRecorder,
    ) -> Result<PublicResponse, PublicError> {
        let planned = planning::plan(
            snapshot.as_ref(),
            request,
            adapter.as_ref(),
            self.protocols.as_ref(),
            self.providers.as_ref(),
        )
        .await?;
        recorder.set_route(planned.public_model.clone(), planned.decoded.stream);
        retry::execute(
            snapshot,
            Arc::clone(&self.protocols),
            planned,
            self.providers.as_ref(),
            self.transport.as_ref(),
            recorder,
        )
        .await
    }
}

pub(super) struct SelectedCandidate {
    pub(super) candidate: RouteCandidate,
    pub(super) permit: RequestPermit,
    pub(super) health: crate::health::AttemptHealth,
}

pub(super) enum RequestPermit {
    Generation(ConcurrencyPermit),
    Auxiliary(AuxiliaryPermit),
}

impl RequestPermit {
    pub(super) fn provider_credential_headers(
        &self,
        driver: &dyn ProviderDriver,
    ) -> Result<CredentialHeaders, ProviderError> {
        match self {
            Self::Generation(permit) => permit.provider_credential_headers(driver),
            Self::Auxiliary(permit) => permit.provider_credential_headers(driver),
        }
    }
}

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
