use std::sync::Arc;

use any2api_domain::{ProtocolDialect, ProtocolOperation};
use serde_json::Value;

use crate::{
    ProtocolError,
    api::{
        DecodedRequest, MAX_BRIDGE_CONTINUATION_STATE_BYTES, ProtocolBridgeContext,
        ResumableProtocolContinuation, StartedProtocolBridge,
    },
};

use super::{bridge::start_session, tool_projection::ToolProjection};

pub(super) struct ResponsesChatContinuation {
    conversation: Arc<[Value]>,
    projection: ToolProjection,
    serialized_bytes: usize,
}

impl ResponsesChatContinuation {
    pub(super) fn new(
        conversation: Vec<Value>,
        projection: ToolProjection,
    ) -> Result<Self, ProtocolError> {
        let serialized_bytes = serialized_json_bytes(&conversation)?
            .checked_add(projection.serialized_bytes())
            .ok_or(ProtocolError::ContinuationTooLarge {
                bytes: usize::MAX,
                max_bytes: MAX_BRIDGE_CONTINUATION_STATE_BYTES,
            })?;
        if serialized_bytes > MAX_BRIDGE_CONTINUATION_STATE_BYTES {
            return Err(ProtocolError::ContinuationTooLarge {
                bytes: serialized_bytes,
                max_bytes: MAX_BRIDGE_CONTINUATION_STATE_BYTES,
            });
        }
        Ok(Self {
            conversation: conversation.into(),
            projection,
            serialized_bytes,
        })
    }

    pub(super) fn conversation(&self) -> &[Value] {
        &self.conversation
    }

    pub(super) const fn projection(&self) -> &ToolProjection {
        &self.projection
    }
}

impl ResumableProtocolContinuation for ResponsesChatContinuation {
    fn ingress_dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiResponses
    }

    fn upstream_dialect(&self) -> ProtocolDialect {
        ProtocolDialect::OpenAiChatCompletions
    }

    fn operation(&self) -> ProtocolOperation {
        ProtocolOperation::Responses
    }

    fn serialized_bytes(&self) -> usize {
        self.serialized_bytes
    }

    fn resume(
        &self,
        request: &DecodedRequest,
        context: ProtocolBridgeContext<'_>,
    ) -> Result<StartedProtocolBridge, ProtocolError> {
        let profile = context
            .target_profile
            .openai_chat_completions()
            .ok_or_else(|| ProtocolError::Unsupported("Chat target profile is required".into()))?;
        start_session(request, context.upstream_model, profile, Some(self))
    }
}

pub(super) fn serialized_json_bytes(value: &[Value]) -> Result<usize, ProtocolError> {
    struct CountingWriter(usize);

    impl std::io::Write for CountingWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0 = self.0.saturating_add(buffer.len());
            Ok(buffer.len())
        }

        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }

    let mut writer = CountingWriter(0);
    serde_json::to_writer(&mut writer, value).map_err(|_| {
        ProtocolError::InvalidPayload("continuation state is not serializable".into())
    })?;
    Ok(writer.0)
}
