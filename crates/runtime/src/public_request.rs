mod planning;
mod response;
mod upstream;

use std::sync::Arc;

use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError};
use any2api_protocol::api::{EgressResponse, ProtocolAdapter, ProtocolRegistry};
use any2api_provider::api::ProviderRegistry;
use any2api_transport::api::TransportManager;
use bytes::Bytes;
use http::{HeaderMap, StatusCode};
use thiserror::Error;

use crate::{published_snapshot::PublishedSnapshot, route_candidates::RouteCandidate};

#[derive(Clone)]
pub struct PublicRequest {
    pub operation: ProtocolOperation,
    pub headers: HeaderMap,
    pub body: Bytes,
}

pub struct PublicResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: Bytes,
}

pub struct PublicRequestService {
    protocols: Arc<ProtocolRegistry>,
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
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
        })
    }

    pub async fn execute(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        request: PublicRequest,
    ) -> PublicResponse {
        let adapter = Arc::clone(
            self.protocols
                .get(request.operation.dialect())
                .expect("validated protocol registry"),
        );
        let result = self
            .execute_inner(snapshot, request, adapter.as_ref())
            .await;
        match result {
            Ok(response) => response.into(),
            Err(error) => adapter.error_response(&error).into(),
        }
    }

    async fn execute_inner(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        request: PublicRequest,
        adapter: &dyn ProtocolAdapter,
    ) -> Result<EgressResponse, PublicError> {
        let planned = planning::plan(snapshot.as_ref(), request, adapter, self.providers.as_ref())?;
        upstream::execute_attempt(
            snapshot.as_ref(),
            adapter,
            planned.decoded,
            &planned.public_model,
            planned.selected,
            self.providers.as_ref(),
            self.transport.as_ref(),
        )
        .await
    }
}

pub(super) struct SelectedCandidate {
    pub(super) candidate: RouteCandidate,
    pub(super) permit: crate::credential_runtime::ConcurrencyPermit,
}

impl From<EgressResponse> for PublicResponse {
    fn from(response: EgressResponse) -> Self {
        Self {
            status: response.status,
            headers: response.headers,
            body: response.body,
        }
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum PublicRequestServiceError {
    #[error("missing protocol adapter for {0:?}")]
    MissingProtocol(ProtocolDialect),
}
