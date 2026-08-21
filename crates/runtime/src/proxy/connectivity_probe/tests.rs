use std::sync::{Arc, Mutex};

use any2api_domain::ProxyProfileId;
use any2api_storage::api::{ConfigurationRepository, SqliteStore};
use any2api_transport::api::{
    TransportFailureScope, TransportManager, TransportProxy, TransportRequest, TransportResponse,
    TransportTrafficClass,
};
use async_trait::async_trait;
use http::{HeaderMap, Method, StatusCode};
use tempfile::tempdir;

use crate::{
    configuration::PublishedSnapshot,
    proxy::{ProxyTestFailureScope, ProxyTestOutcome, ProxyTestService},
    registry::RuntimeRegistry,
};

#[tokio::test]
async fn connectivity_probe_uses_the_fixed_public_target_without_provider_configuration() {
    let directory = tempdir().expect("temporary directory");
    let storage = SqliteStore::connect(&directory.path().join("config.sqlite3"))
        .await
        .expect("storage");
    let configuration = storage.load_configuration().await.expect("configuration");
    assert!(configuration.provider_endpoints().endpoints().is_empty());
    let snapshot = Arc::new(
        PublishedSnapshot::new(
            configuration,
            &RuntimeRegistry::new(),
            crate::test_support::configuration_capabilities().provider_registry(),
        )
        .expect("initial snapshot"),
    );
    let transport = Arc::new(CapturingTransport::default());
    let service = ProxyTestService::new(Arc::clone(&transport) as Arc<dyn TransportManager>);

    let result = service
        .test(snapshot, ProxyProfileId::DIRECT)
        .await
        .expect("proxy test");

    assert_eq!(
        result.outcome(),
        ProxyTestOutcome::Reachable { status_code: 204 }
    );
    let captured = transport.request.lock().expect("captured request");
    let request = captured.as_ref().expect("connectivity probe request");
    assert_eq!(request.method, Method::GET);
    assert_eq!(request.uri, "https://example.com/");
    assert!(request.headers.is_empty());
    assert!(request.body.is_empty());
    assert_eq!(
        request.isolation.traffic_class(),
        TransportTrafficClass::Diagnostic
    );
    assert_eq!(request.read_timeout, std::time::Duration::from_secs(10));
    assert_eq!(
        *transport.proxy_id.lock().expect("captured proxy"),
        Some(ProxyProfileId::DIRECT)
    );
}

#[test]
fn transport_endpoint_failures_are_named_as_probe_target_failures() {
    assert_eq!(
        ProxyTestFailureScope::from(TransportFailureScope::Endpoint).as_str(),
        "probe_target"
    );
}

#[derive(Default)]
struct CapturingTransport {
    request: Mutex<Option<TransportRequest>>,
    proxy_id: Mutex<Option<ProxyProfileId>>,
}

#[async_trait]
impl TransportManager for CapturingTransport {
    async fn execute(
        &self,
        proxy: TransportProxy<'_>,
        request: TransportRequest,
    ) -> Result<TransportResponse, any2api_transport::api::TransportError> {
        *self.request.lock().expect("captured request") = Some(request);
        *self.proxy_id.lock().expect("captured proxy") = Some(proxy.profile().id());
        Ok(TransportResponse {
            status: StatusCode::NO_CONTENT,
            headers: HeaderMap::new(),
            body: Box::pin(futures_util::stream::empty()),
            read_failure_scope: TransportFailureScope::Endpoint,
        })
    }
}
