use std::{collections::HashMap, sync::Arc};

use any2api_domain::{ProtocolDialect, ProtocolOperation};

use crate::{
    ProtocolError,
    api::{ProtocolAdapter, ProtocolBridge, ProtocolExchange},
};

#[derive(Default)]
pub struct ProtocolRegistry {
    adapters: HashMap<ProtocolDialect, Arc<dyn ProtocolAdapter>>,
    bridges: HashMap<(ProtocolDialect, ProtocolDialect), Arc<dyn ProtocolBridge>>,
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

    pub fn register_bridge(
        &mut self,
        bridge: Arc<dyn ProtocolBridge>,
    ) -> Result<(), ProtocolError> {
        let key = (bridge.ingress_dialect(), bridge.upstream_dialect());
        if self.bridges.contains_key(&key) {
            return Err(ProtocolError::DuplicateBridge(key.0, key.1));
        }
        self.bridges.insert(key, bridge);
        Ok(())
    }

    #[must_use]
    pub fn get(&self, dialect: ProtocolDialect) -> Option<&Arc<dyn ProtocolAdapter>> {
        self.adapters.get(&dialect)
    }

    pub fn iter(&self) -> impl Iterator<Item = (&ProtocolDialect, &Arc<dyn ProtocolAdapter>)> {
        self.adapters.iter()
    }

    #[must_use]
    pub fn supports_pair(&self, ingress: ProtocolDialect, upstream: ProtocolDialect) -> bool {
        self.adapters.contains_key(&ingress)
            && self.adapters.contains_key(&upstream)
            && (ingress == upstream || self.bridges.contains_key(&(ingress, upstream)))
    }

    #[must_use]
    pub fn supports_operation(
        &self,
        ingress: ProtocolDialect,
        upstream: ProtocolDialect,
        operation: ProtocolOperation,
    ) -> bool {
        if operation.dialect() != ingress || !self.supports_pair(ingress, upstream) {
            return false;
        }
        ingress == upstream
            || self
                .bridges
                .get(&(ingress, upstream))
                .is_some_and(|bridge| bridge.supports_operation(operation))
    }

    pub fn exchange(
        &self,
        ingress: ProtocolDialect,
        upstream: ProtocolDialect,
        operation: ProtocolOperation,
    ) -> Result<ProtocolExchange, ProtocolError> {
        if !self.supports_operation(ingress, upstream, operation) {
            return Err(ProtocolError::Unsupported(format!(
                "{operation:?}: {ingress:?} -> {upstream:?}"
            )));
        }
        let ingress_adapter = self
            .adapters
            .get(&ingress)
            .cloned()
            .ok_or_else(|| ProtocolError::Unsupported(format!("{ingress:?}")))?;
        if ingress == upstream {
            return Ok(ProtocolExchange::direct(ingress_adapter, operation));
        }
        let upstream_adapter = self
            .adapters
            .get(&upstream)
            .cloned()
            .ok_or_else(|| ProtocolError::Unsupported(format!("{upstream:?}")))?;
        let bridge = self
            .bridges
            .get(&(ingress, upstream))
            .cloned()
            .ok_or_else(|| ProtocolError::Unsupported(format!("{ingress:?} -> {upstream:?}")))?;
        Ok(ProtocolExchange::bridged(
            ingress_adapter,
            upstream_adapter,
            bridge,
            operation,
        ))
    }

    pub fn iter_bridges(
        &self,
    ) -> impl Iterator<
        Item = (
            &(ProtocolDialect, ProtocolDialect),
            &Arc<dyn ProtocolBridge>,
        ),
    > {
        self.bridges.iter()
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use any2api_domain::{ProtocolDialect, ProtocolOperation, PublicError};
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
            _operation: ProtocolOperation,
            _headers: HeaderMap,
            _payload: AdapterPayload,
            _upstream_model: &str,
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

        fn encode_egress_event(
            &self,
            _event: AdapterEvent,
            _public_model: &str,
        ) -> Result<SseFrame, ProtocolError> {
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
