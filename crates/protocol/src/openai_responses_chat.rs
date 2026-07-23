use std::sync::Arc;

use any2api_domain::{ProtocolDialect, ProtocolOperation};
use bytes::Bytes;
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

#[path = "openai_responses_chat/history.rs"]
mod history;
#[path = "openai_responses_chat/request.rs"]
mod request;
#[path = "openai_responses_chat/response.rs"]
mod response;
#[path = "openai_responses_chat/stream.rs"]
mod stream;
#[cfg(test)]
#[path = "openai_responses_chat/tests.rs"]
mod tests;

use history::ChatHistoryStore;
use stream::ChatToResponsesStream;

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
        let AdapterPayload::RawJson(raw) = decoded.payload;
        let value: Value = serde_json::from_slice(&raw)
            .map_err(|_| ProtocolError::InvalidPayload("request body must be valid JSON".into()))?;
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
        let body = serde_json::to_vec(&converted.body)
            .map(Bytes::from)
            .map_err(|_| {
                ProtocolError::InvalidPayload("request JSON could not be encoded".into())
            })?;
        let request = json_codec::encode_request(
            ProtocolOperation::ChatCompletions,
            decoded.headers,
            AdapterPayload::RawJson(body),
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
        let AdapterPayload::RawJson(raw) = decoded.payload;
        let value = serde_json::from_slice(&raw).map_err(|_| {
            ProtocolError::InvalidPayload("Chat Completions response must be valid JSON".into())
        })?;
        let converted = response::convert(value, &self.response_id)?;
        self.store(converted.assistant_message);
        decoded.payload =
            AdapterPayload::RawJson(Bytes::from(serde_json::to_vec(&converted.body).map_err(
                |_| ProtocolError::InvalidPayload("Responses JSON could not be encoded".into()),
            )?));
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
