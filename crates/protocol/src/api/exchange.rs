use std::sync::Arc;

use any2api_domain::{
    OpenAiChatCompletionsProfile, ProtocolDialect, ProtocolOperation, ProtocolTargetProfile,
};

use super::{
    AdapterEvent, BridgeContinuationState, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
    EncodedUpstreamRequest, ProtocolAdapter, ProtocolBridge, ProtocolBridgeContext,
    ProtocolBridgeSession, ProtocolContinuationState, ProtocolUpstreamFailureEvidence, SseFrame,
    StreamCompletionPolicy, UpstreamResponse,
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
        let target_profile = match self.upstream.dialect() {
            ProtocolDialect::OpenAiChatCompletions => {
                Some(ProtocolTargetProfile::OpenAiChatCompletions(
                    OpenAiChatCompletionsProfile::COMPATIBLE_BASELINE,
                ))
            }
            _ => None,
        };
        self.prepare_request_with_target_profile(
            request,
            upstream_model,
            target_profile,
            continuation,
        )
    }

    pub fn prepare_request_with_target_profile(
        &mut self,
        request: &DecodedRequest,
        upstream_model: &str,
        target_profile: Option<ProtocolTargetProfile>,
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
        let target_profile = target_profile.ok_or_else(|| {
            ProtocolError::Unsupported("bridge target profile is unavailable".into())
        })?;
        let context = ProtocolBridgeContext::new(upstream_model, target_profile);
        let started = match continuation {
            Some(continuation) => continuation.resume(
                self.ingress.dialect(),
                self.upstream.dialect(),
                self.operation,
                request,
                context,
            )?,
            None => bridge.start(request, context)?,
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
        let decoded = if self.bridge.is_none() {
            self.upstream.decode_direct_upstream_response(response)?
        } else {
            self.upstream.decode_upstream_response(response)?
        };
        match self.bridge_session.as_mut() {
            Some(session) => session.transform_response(decoded),
            None if self.bridge.is_none() => Ok(decoded),
            None => Err(ProtocolError::InvalidPayload(
                "protocol bridge session was not prepared".into(),
            )),
        }
    }

    #[must_use]
    pub fn buffered_upstream_failure(
        &self,
        response: &UpstreamResponse,
    ) -> Option<ProtocolUpstreamFailureEvidence> {
        self.upstream.buffered_upstream_failure(response)
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

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use any2api_domain::{ProtocolOperation, TokenUsage};
    use bytes::Bytes;
    use http::{HeaderMap, StatusCode};

    use super::ProtocolExchange;
    use crate::{
        AnthropicMessagesAdapter, OpenAiChatCompletionsAdapter, OpenAiImagesAdapter,
        OpenAiResponsesAdapter,
        api::{DecodedResponsePayload, ProtocolAdapter, UpstreamResponse},
    };

    #[test]
    fn direct_exchange_keeps_validated_wire_json_without_a_structured_tree() {
        let adapter: Arc<dyn ProtocolAdapter> = Arc::new(OpenAiResponsesAdapter::new());
        let mut exchange = ProtocolExchange::direct(adapter, ProtocolOperation::Responses);
        let body = Bytes::from_static(
            br#"{"id":"resp_1","model":"public","output":[{"content":"large payload"}],"usage":{"input_tokens":12,"output_tokens":7,"input_tokens_details":{"cached_tokens":3}}}"#,
        );
        let body_pointer = body.as_ptr();
        let decoded = exchange
            .decode_upstream_response(UpstreamResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body,
            })
            .expect("direct response");

        assert_eq!(
            decoded.telemetry.token_usage,
            TokenUsage::new(Some(12), Some(7), Some(3))
        );
        let DecodedResponsePayload::RawJson(wire_body) = &decoded.payload else {
            panic!("direct decode must not materialize a JSON tree");
        };
        assert_eq!(wire_body.as_ptr(), body_pointer);
        assert_eq!(
            exchange
                .continuation_id_from_response(ProtocolOperation::Responses, &decoded)
                .expect("continuation identity"),
            Some("resp_1".into())
        );

        let encoded = exchange
            .encode_egress_response(decoded, "public")
            .expect("egress response");
        assert_eq!(encoded.body.as_ptr(), body_pointer);
    }

    #[test]
    fn direct_exchange_still_rejects_malformed_json() {
        let adapter: Arc<dyn ProtocolAdapter> = Arc::new(OpenAiResponsesAdapter::new());
        let mut exchange = ProtocolExchange::direct(adapter, ProtocolOperation::Responses);

        assert!(
            exchange
                .decode_upstream_response(UpstreamResponse {
                    status: StatusCode::OK,
                    headers: HeaderMap::new(),
                    body: Bytes::from_static(br#"{"output":"unterminated}"#),
                })
                .is_err()
        );
    }

    #[test]
    fn exchange_exposes_only_exact_buffered_failures_before_decode_or_bridge() {
        type FailureCase = (
            Arc<dyn ProtocolAdapter>,
            ProtocolOperation,
            &'static [u8],
            bool,
        );
        let cases: Vec<FailureCase> = vec![
            (
                Arc::new(OpenAiResponsesAdapter::new()),
                ProtocolOperation::Responses,
                br#"{"status":"failed","error":{"code":"server_error"}}"#,
                true,
            ),
            (
                Arc::new(OpenAiResponsesAdapter::new()),
                ProtocolOperation::Responses,
                br#"{"status":"incomplete","error":{"code":"server_error"}}"#,
                false,
            ),
            (
                Arc::new(OpenAiChatCompletionsAdapter::new()),
                ProtocolOperation::ChatCompletions,
                br#"{"error":{"code":"server_error"}}"#,
                true,
            ),
            (
                Arc::new(OpenAiImagesAdapter::new()),
                ProtocolOperation::ImagesGenerations,
                br#"{"error":{"code":"server_error"}}"#,
                true,
            ),
            (
                Arc::new(AnthropicMessagesAdapter::new()),
                ProtocolOperation::Messages,
                br#"{"type":"error","error":{"type":"api_error"}}"#,
                true,
            ),
            (
                Arc::new(AnthropicMessagesAdapter::new()),
                ProtocolOperation::Messages,
                br#"{"type":"message","error":{"type":"api_error"}}"#,
                false,
            ),
        ];

        for (adapter, operation, bytes, expected) in cases {
            let exchange = ProtocolExchange::direct(adapter, operation);
            let body = Bytes::from_static(bytes);
            let response = UpstreamResponse {
                status: StatusCode::OK,
                headers: HeaderMap::new(),
                body: body.clone(),
            };
            let evidence = exchange.buffered_upstream_failure(&response);
            assert_eq!(evidence.is_some(), expected, "body={body:?}");
            if let Some(evidence) = evidence {
                assert_eq!(evidence.raw_json().as_ptr(), body.as_ptr());
                assert_eq!(evidence.retry_safety_override(), None);
            }
        }

        let exchange = ProtocolExchange::direct(
            Arc::new(OpenAiResponsesAdapter::new()),
            ProtocolOperation::Responses,
        );
        assert!(
            exchange
                .buffered_upstream_failure(&UpstreamResponse {
                    status: StatusCode::INTERNAL_SERVER_ERROR,
                    headers: HeaderMap::new(),
                    body: Bytes::from_static(
                        br#"{"status":"failed","error":{"code":"server_error"}}"#,
                    ),
                })
                .is_none(),
            "buffered protocol evidence is reserved for successful HTTP statuses"
        );
    }
}
