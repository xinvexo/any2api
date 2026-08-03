use std::sync::Arc;

use any2api_domain::ProtocolOperation;

use super::{
    AdapterEvent, BridgeContinuationState, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
    EncodedUpstreamRequest, ProtocolAdapter, ProtocolBridge, ProtocolBridgeSession,
    ProtocolContinuationState, SseFrame, StreamCompletionPolicy, UpstreamResponse,
};
use crate::ProtocolError;

pub struct PreparedProtocolRequest {
    pub upstream_operation: ProtocolOperation,
    pub request: EncodedUpstreamRequest,
}

pub struct StartedProtocolBridge {
    upstream_operation: ProtocolOperation,
    request: EncodedUpstreamRequest,
    session: Box<dyn ProtocolBridgeSession>,
}

impl StartedProtocolBridge {
    #[must_use]
    pub fn new(
        upstream_operation: ProtocolOperation,
        request: EncodedUpstreamRequest,
        session: Box<dyn ProtocolBridgeSession>,
    ) -> Self {
        Self {
            upstream_operation,
            request,
            session,
        }
    }

    fn into_parts(
        self,
    ) -> (
        ProtocolOperation,
        EncodedUpstreamRequest,
        Box<dyn ProtocolBridgeSession>,
    ) {
        (self.upstream_operation, self.request, self.session)
    }
}

pub struct ProtocolExchange {
    ingress: Arc<dyn ProtocolAdapter>,
    upstream: Arc<dyn ProtocolAdapter>,
    operation: ProtocolOperation,
    upstream_operation: ProtocolOperation,
    bridge: Option<Arc<dyn ProtocolBridge>>,
    bridge_session: Option<Box<dyn ProtocolBridgeSession>>,
}

impl ProtocolExchange {
    pub(crate) fn direct(adapter: Arc<dyn ProtocolAdapter>, operation: ProtocolOperation) -> Self {
        Self {
            ingress: Arc::clone(&adapter),
            upstream: adapter,
            operation,
            upstream_operation: operation,
            bridge: None,
            bridge_session: None,
        }
    }

    pub(crate) fn bridged(
        ingress: Arc<dyn ProtocolAdapter>,
        upstream: Arc<dyn ProtocolAdapter>,
        bridge: Arc<dyn ProtocolBridge>,
        operation: ProtocolOperation,
    ) -> Self {
        Self {
            ingress,
            upstream,
            operation,
            upstream_operation: operation,
            bridge: Some(bridge),
            bridge_session: None,
        }
    }

    pub fn prepare_request(
        &mut self,
        request: &DecodedRequest,
        upstream_model: &str,
        continuation: Option<ProtocolContinuationState>,
    ) -> Result<PreparedProtocolRequest, ProtocolError> {
        if request.dialect != self.ingress.dialect() || request.operation != self.operation {
            return Err(ProtocolError::Unsupported(format!(
                "{:?} request on {:?} exchange",
                request.operation,
                self.ingress.dialect()
            )));
        }
        let Some(bridge) = &self.bridge else {
            if continuation.is_some() {
                return Err(ProtocolError::SessionBindingLost);
            }
            let operation = request.operation;
            let encoded = self.ingress.encode_upstream_request(
                operation,
                &request.headers,
                &request.payload,
                upstream_model,
            )?;
            return Ok(PreparedProtocolRequest {
                upstream_operation: operation,
                request: encoded,
            });
        };
        let started = match continuation {
            Some(continuation) => continuation.resume(
                self.ingress.dialect(),
                self.upstream.dialect(),
                self.operation,
                request,
                upstream_model,
            )?,
            None => bridge.start(request, upstream_model)?,
        };
        let (upstream_operation, request, session) = started.into_parts();
        self.upstream_operation = upstream_operation;
        self.bridge_session = Some(session);
        Ok(PreparedProtocolRequest {
            upstream_operation,
            request,
        })
    }

    pub fn decode_upstream_response(
        &mut self,
        response: UpstreamResponse,
    ) -> Result<DecodedUpstreamResponse, ProtocolError> {
        let decoded = self.upstream.decode_upstream_response(response)?;
        match self.bridge_session.as_mut() {
            Some(session) => session.transform_response(decoded),
            None if self.bridge.is_none() => Ok(decoded),
            None => Err(ProtocolError::InvalidPayload(
                "protocol bridge session was not prepared".into(),
            )),
        }
    }

    pub fn decode_upstream_event(
        &mut self,
        frame: SseFrame,
    ) -> Result<Vec<AdapterEvent>, ProtocolError> {
        let event = self.upstream.decode_upstream_event(frame)?;
        match self.bridge_session.as_mut() {
            Some(session) => session.transform_event(event),
            None if self.bridge.is_none() => Ok(vec![event]),
            None => Err(ProtocolError::InvalidPayload(
                "protocol bridge session was not prepared".into(),
            )),
        }
    }

    pub fn finish_upstream_events(&mut self) -> Result<Vec<AdapterEvent>, ProtocolError> {
        match self.bridge_session.as_mut() {
            Some(session) => session.finish_events(),
            None => Ok(Vec::new()),
        }
    }

    #[must_use]
    pub fn bridge_continuation_state(&self) -> BridgeContinuationState {
        self.bridge_session
            .as_ref()
            .map_or(BridgeContinuationState::Stateless, |session| {
                session.continuation_state()
            })
    }

    #[must_use]
    pub fn stream_completion_policy(&self) -> StreamCompletionPolicy {
        self.upstream
            .stream_completion_policy(self.upstream_operation)
    }

    pub fn encode_egress_response(
        &self,
        response: DecodedUpstreamResponse,
        public_model: &str,
    ) -> Result<EgressResponse, ProtocolError> {
        self.ingress.encode_egress_response(response, public_model)
    }

    pub fn encode_egress_event(
        &self,
        event: AdapterEvent,
        public_model: &str,
    ) -> Result<SseFrame, ProtocolError> {
        self.ingress.encode_egress_event(event, public_model)
    }

    pub fn continuation_id_from_response(
        &self,
        operation: ProtocolOperation,
        response: &DecodedUpstreamResponse,
    ) -> Result<Option<String>, ProtocolError> {
        self.ingress
            .continuation_id_from_response(operation, response)
    }

    pub fn continuation_id_from_event(
        &self,
        operation: ProtocolOperation,
        event: &AdapterEvent,
    ) -> Result<Option<String>, ProtocolError> {
        self.ingress.continuation_id_from_event(operation, event)
    }
}
