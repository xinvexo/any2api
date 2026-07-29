use std::sync::Arc;

use any2api_domain::{ProtocolDialect, ProtocolOperation};
use serde_json::Value;
use uuid::Uuid;

use crate::{
    ProtocolError,
    api::{
        AdapterEvent, AdapterPayload, DecodedRequest, DecodedUpstreamResponse, ProtocolBridge,
        ProtocolBridgeSession, StartedProtocolBridge,
    },
    json_codec,
};

use super::{history::ChatHistoryStore, request, response, stream::ChatToResponsesStream};

#[derive(Default)]
pub struct ResponsesToChatCompletionsBridge {
    history: Arc<ChatHistoryStore>,
}

impl ResponsesToChatCompletionsBridge {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}

impl ProtocolBridge for ResponsesToChatCompletionsBridge {
    fn ingress_dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiResponses
    }

    fn upstream_dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiChatCompletions
    }

    fn supports_operation(&self, operation: ProtocolOperation) -> bool {
        operation == ProtocolOperation::Responses
    }

    fn start(
        &self,
        decoded: DecodedRequest,
        upstream_model: &str,
    ) -> Result<StartedProtocolBridge, ProtocolError> {
        if decoded.operation != ProtocolOperation::Responses {
            return Err(ProtocolError::Unsupported(format!(
                "{:?}",
                decoded.operation
            )));
        }
        let AdapterPayload::Json(value) = decoded.payload else {
            return Err(ProtocolError::InvalidPayload(
                "Responses bridge requires a JSON request body".into(),
            ));
        };
        let previous = match value.get("previous_response_id") {
            Some(Value::String(id)) if !id.is_empty() => self
                .history
                .get(id)
                .ok_or(ProtocolError::SessionBindingLost)?,
            Some(Value::Null) | None => Vec::new(),
            Some(_) => {
                return Err(ProtocolError::InvalidPayload(
                    "previous_response_id must be a string".into(),
                ));
            }
        };
        let converted = request::convert(value, upstream_model, previous)?;
        let request = json_codec::encode_request(
            ProtocolOperation::ChatCompletions,
            decoded.headers,
            AdapterPayload::Json(converted.body),
            upstream_model,
        )?;
        let response_id = format!("resp_{}", Uuid::new_v4().simple());
        Ok(StartedProtocolBridge::new(
            ProtocolOperation::ChatCompletions,
            request,
            Box::new(ResponsesToChatSession {
                history: Arc::clone(&self.history),
                conversation: converted.conversation,
                stream: ChatToResponsesStream::new(response_id.clone(), upstream_model.into()),
                response_id,
                stored: false,
            }),
        ))
    }
}

struct ResponsesToChatSession {
    history: Arc<ChatHistoryStore>,
    conversation: Vec<Value>,
    stream: ChatToResponsesStream,
    response_id: String,
    stored: bool,
}

impl ResponsesToChatSession {
    fn store(&mut self, assistant_message: Value) {
        if self.stored {
            return;
        }
        self.conversation.push(assistant_message);
        self.history
            .insert(self.response_id.clone(), self.conversation.clone());
        self.stored = true;
    }
}

impl ProtocolBridgeSession for ResponsesToChatSession {
    fn transform_response(
        &mut self,
        mut decoded: DecodedUpstreamResponse,
    ) -> Result<DecodedUpstreamResponse, ProtocolError> {
        let converted = response::convert(decoded.parsed, &self.response_id)?;
        self.store(converted.assistant_message);
        decoded.parsed = converted.body;
        decoded.body = None;
        Ok(decoded)
    }

    fn transform_event(&mut self, event: AdapterEvent) -> Result<Vec<AdapterEvent>, ProtocolError> {
        let update = self.stream.push(event)?;
        if let Some(message) = update.assistant_message {
            self.store(message);
        }
        Ok(update.events)
    }

    fn finish_events(&mut self) -> Result<Vec<AdapterEvent>, ProtocolError> {
        let update = self.stream.finish()?;
        if let Some(message) = update.assistant_message {
            self.store(message);
        }
        Ok(update.events)
    }
}

impl std::fmt::Debug for ResponsesToChatCompletionsBridge {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("ResponsesToChatCompletionsBridge")
            .finish_non_exhaustive()
    }
}
