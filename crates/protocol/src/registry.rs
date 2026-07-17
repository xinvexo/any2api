use std::{collections::HashMap, sync::Arc};

use any2api_domain::ProtocolDialect;

use crate::{ProtocolError, api::ProtocolAdapter};

#[derive(Default)]
pub struct ProtocolRegistry {
    adapters: HashMap<ProtocolDialect, Arc<dyn ProtocolAdapter>>,
}

impl ProtocolRegistry {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn register(&mut self, adapter: Arc<dyn ProtocolAdapter>) -> Result<(), ProtocolError> {
        let dialect = adapter.dialect();
        if self.adapters.contains_key(&dialect) {
            return Err(ProtocolError::DuplicateDialect(dialect));
        }

        self.adapters.insert(dialect, adapter);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, dialect: ProtocolDialect) -> Option<&Arc<dyn ProtocolAdapter>> {
        self.adapters.get(&dialect)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ProtocolDialect, &Arc<dyn ProtocolAdapter>)> {
        self.adapters.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use any2api_domain::{ProtocolDialect, PublicError};
    use bytes::Bytes;
    use http::{HeaderMap, StatusCode};

    use super::ProtocolRegistry;
    use crate::{
        ProtocolError,
        api::{
            AdapterEvent, AdapterPayload, DecodedRequest, DecodedUpstreamResponse, EgressResponse,
            EncodedUpstreamRequest, IngressRequest, ProtocolAdapter, SseFrame, UpstreamResponse,
        },
    };

    struct FakeAdapter;

    impl ProtocolAdapter for FakeAdapter {
        fn dialect(&self) -> ProtocolDialect {
            ProtocolDialect::OpenAiResponses
        }

        fn decode_ingress_request(
            &self,
            _request: IngressRequest,
        ) -> Result<DecodedRequest, ProtocolError> {
            Err(ProtocolError::Unsupported("test".into()))
        }

        fn encode_upstream_request(
            &self,
            _payload: AdapterPayload,
        ) -> Result<EncodedUpstreamRequest, ProtocolError> {
            Err(ProtocolError::Unsupported("test".into()))
        }

        fn decode_upstream_response(
            &self,
            _response: UpstreamResponse,
        ) -> Result<DecodedUpstreamResponse, ProtocolError> {
            Err(ProtocolError::Unsupported("test".into()))
        }

        fn decode_upstream_event(&self, _frame: SseFrame) -> Result<AdapterEvent, ProtocolError> {
            Err(ProtocolError::Unsupported("test".into()))
        }

        fn encode_egress_response(
            &self,
            _response: DecodedUpstreamResponse,
        ) -> Result<EgressResponse, ProtocolError> {
            Err(ProtocolError::Unsupported("test".into()))
        }

        fn encode_egress_event(&self, _event: AdapterEvent) -> Result<SseFrame, ProtocolError> {
            Err(ProtocolError::Unsupported("test".into()))
        }

        fn error_response(&self, _error: &PublicError) -> EgressResponse {
            EgressResponse {
                status: StatusCode::INTERNAL_SERVER_ERROR,
                headers: HeaderMap::new(),
                body: Bytes::new(),
            }
        }
    }

    #[test]
    fn duplicate_dialects_are_rejected() {
        let mut registry = ProtocolRegistry::new();
        registry
            .register(Arc::new(FakeAdapter))
            .expect("first adapter registers");

        let error = registry
            .register(Arc::new(FakeAdapter))
            .expect_err("duplicate adapter must fail");

        assert_eq!(
            error,
            ProtocolError::DuplicateDialect(ProtocolDialect::OpenAiResponses)
        );
    }
}
