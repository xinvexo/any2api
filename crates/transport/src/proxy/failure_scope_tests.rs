use std::{str::FromStr, time::Duration};

use any2api_domain::{
    ProxyAddress, ProxyDraft, ProxyKind, ProxyProfile, ProxyProfileId, RetrySafety,
};
use bytes::Bytes;
use http::{HeaderMap, Method, Uri};
use tokio::net::TcpListener;

use crate::{
    ReqwestTransportManager,
    api::{
        EndpointNetworkPolicy, TransportManager, TransportManagerConfig, TransportProxy,
        TransportRequest,
    },
    error::{TransportErrorStage, TransportFailureScope},
};

#[tokio::test]
async fn plain_http_proxy_connection_failure_is_attributed_to_the_proxy() {
    let unavailable = TcpListener::bind("127.0.0.1:0")
        .await
        .expect("temporary proxy listener");
    let unavailable_address = unavailable.local_addr().expect("proxy address");
    drop(unavailable);
    let manager = ReqwestTransportManager::new(TransportManagerConfig {
        connect_timeout: Duration::from_millis(500),
        ..TransportManagerConfig::default()
    })
    .expect("transport manager");
    let proxy = network_proxy(unavailable_address);

    let error = match manager
        .execute(
            TransportProxy::new(&proxy, None),
            request_to("http://upstream.invalid/proxy-connect"),
        )
        .await
    {
        Ok(_) => panic!("unavailable proxy must fail"),
        Err(error) => error,
    };

    assert_eq!(error.stage, TransportErrorStage::ProxyHandshake);
    assert_eq!(error.failure_scope, TransportFailureScope::Proxy);
    assert_eq!(error.retry_safety, RetrySafety::DefinitelyNotSent);
}

fn request_to(uri: &str) -> TransportRequest {
    TransportRequest {
        method: Method::GET,
        uri: Uri::from_str(uri).expect("request URI"),
        headers: HeaderMap::new(),
        body: Bytes::new(),
        network_policy: EndpointNetworkPolicy::new(),
        read_timeout: Duration::from_secs(15),
    }
}

fn network_proxy(address: std::net::SocketAddr) -> ProxyProfile {
    let address =
        ProxyAddress::new(address.ip().to_string(), address.port()).expect("proxy address");
    let draft =
        ProxyDraft::new("Unavailable", ProxyKind::Http, address, true).expect("proxy draft");
    ProxyProfile::create(ProxyProfileId::new(), draft).expect("proxy profile")
}
