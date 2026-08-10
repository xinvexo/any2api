use std::{str::FromStr, time::Duration};

use bytes::Bytes;
use http::{HeaderMap, Method, Uri, header};

use crate::api::{
    EndpointNetworkPolicy, TransportIsolationKey, TransportRequest, TransportTrafficClass,
};

pub(super) fn fixture_request(uri: &str) -> TransportRequest {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::AUTHORIZATION,
        "Bearer fixture-token"
            .parse()
            .expect("fixture authorization"),
    );
    headers.insert(
        header::CONTENT_TYPE,
        "application/json".parse().expect("fixture content type"),
    );
    headers.insert(
        "x-wire-fixture",
        "generic-v2".parse().expect("fixture header"),
    );
    TransportRequest {
        method: Method::POST,
        uri: Uri::from_str(uri).expect("fixture URI"),
        headers,
        body: Bytes::from_static(b"{}"),
        isolation: TransportIsolationKey::ephemeral(TransportTrafficClass::Diagnostic),
        network_policy: EndpointNetworkPolicy::new(),
        read_timeout: Duration::from_secs(15),
    }
}

pub(super) fn assert_fixture(actual: &str, expected: &str) {
    assert_eq!(actual, expected, "wire fixture changed; review the diff");
}
