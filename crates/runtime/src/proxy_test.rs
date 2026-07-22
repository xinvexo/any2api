use std::{sync::Arc, time::Instant};

use any2api_domain::{ConfigRevision, ProviderEndpointId, ProxyProfileId};
use any2api_transport::api::{
    EndpointNetworkPolicy, TransportErrorStage, TransportFailureScope, TransportManager,
    TransportRequest,
};
use bytes::Bytes;
use http::{HeaderMap, Method};
use thiserror::Error;

use crate::published_snapshot::PublishedSnapshot;

pub struct ProxyTestService {
    transport: Arc<dyn TransportManager>,
}

impl ProxyTestService {
    #[must_use]
    pub fn new(transport: Arc<dyn TransportManager>) -> Self {
        Self { transport }
    }

    pub async fn test(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        proxy_id: ProxyProfileId,
        endpoint_id: ProviderEndpointId,
    ) -> Result<ProxyTestResult, ProxyTestError> {
        let proxy = snapshot
            .transport_proxy(proxy_id)
            .ok_or(ProxyTestError::ProxyNotFound)?;
        if !proxy.profile().enabled() {
            return Err(ProxyTestError::ProxyDisabled);
        }
        let endpoint = snapshot
            .provider_endpoints()
            .get(endpoint_id)
            .ok_or(ProxyTestError::ProviderEndpointNotFound)?;
        let config_revision = snapshot.revision();
        let proxy_config_version = proxy.profile().config_version();
        let provider_endpoint_config_version = endpoint.config_version();
        let uri = endpoint
            .base_url()
            .as_str()
            .parse()
            .map_err(|_| ProxyTestError::InvalidEndpointUri)?;
        let request = TransportRequest {
            method: Method::GET,
            uri,
            headers: HeaderMap::new(),
            body: Bytes::new(),
            network_policy: EndpointNetworkPolicy::new(endpoint.allow_private_network())
                .with_strict_ssrf(snapshot.settings().upstream().strict_ssrf()),
            read_timeout: std::time::Duration::from_millis(
                snapshot.settings().upstream().read_timeout_secs(),
            ),
        };
        let started = Instant::now();
        let outcome = match self.transport.execute(proxy, request).await {
            Ok(response) => ProxyTestOutcome::Reachable {
                status_code: response.status.as_u16(),
            },
            Err(error) => ProxyTestOutcome::Failed {
                stage: error.stage.into(),
                scope: error.failure_scope.into(),
            },
        };
        Ok(ProxyTestResult {
            config_revision,
            proxy_config_version,
            provider_endpoint_config_version,
            proxy_id,
            provider_endpoint_id: endpoint_id,
            latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            outcome,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ProxyTestResult {
    config_revision: ConfigRevision,
    proxy_config_version: u64,
    provider_endpoint_config_version: u64,
    proxy_id: ProxyProfileId,
    provider_endpoint_id: ProviderEndpointId,
    latency_ms: u64,
    outcome: ProxyTestOutcome,
}

impl ProxyTestResult {
    #[must_use]
    pub const fn config_revision(self) -> ConfigRevision {
        self.config_revision
    }

    #[must_use]
    pub const fn proxy_config_version(self) -> u64 {
        self.proxy_config_version
    }

    #[must_use]
    pub const fn provider_endpoint_config_version(self) -> u64 {
        self.provider_endpoint_config_version
    }

    #[must_use]
    pub const fn proxy_id(self) -> ProxyProfileId {
        self.proxy_id
    }

    #[must_use]
    pub const fn provider_endpoint_id(self) -> ProviderEndpointId {
        self.provider_endpoint_id
    }

    #[must_use]
    pub const fn latency_ms(self) -> u64 {
        self.latency_ms
    }

    #[must_use]
    pub const fn outcome(self) -> ProxyTestOutcome {
        self.outcome
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyTestOutcome {
    Reachable {
        status_code: u16,
    },
    Failed {
        stage: ProxyTestFailureStage,
        scope: ProxyTestFailureScope,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyTestFailureStage {
    Dns,
    Tcp,
    ProxyHandshake,
    Tls,
    WriteRequest,
    AwaitHeaders,
    ReadBody,
}

impl ProxyTestFailureStage {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Dns => "dns",
            Self::Tcp => "tcp",
            Self::ProxyHandshake => "proxy_handshake",
            Self::Tls => "tls",
            Self::WriteRequest => "write_request",
            Self::AwaitHeaders => "await_headers",
            Self::ReadBody => "read_body",
        }
    }
}

impl From<TransportErrorStage> for ProxyTestFailureStage {
    fn from(value: TransportErrorStage) -> Self {
        match value {
            TransportErrorStage::Dns => Self::Dns,
            TransportErrorStage::Tcp => Self::Tcp,
            TransportErrorStage::ProxyHandshake => Self::ProxyHandshake,
            TransportErrorStage::Tls => Self::Tls,
            TransportErrorStage::WriteRequest => Self::WriteRequest,
            TransportErrorStage::AwaitHeaders => Self::AwaitHeaders,
            TransportErrorStage::ReadBody => Self::ReadBody,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProxyTestFailureScope {
    Endpoint,
    Proxy,
    Unattributed,
}

impl ProxyTestFailureScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Endpoint => "endpoint",
            Self::Proxy => "proxy",
            Self::Unattributed => "unattributed",
        }
    }
}

impl From<TransportFailureScope> for ProxyTestFailureScope {
    fn from(value: TransportFailureScope) -> Self {
        match value {
            TransportFailureScope::Endpoint => Self::Endpoint,
            TransportFailureScope::Proxy => Self::Proxy,
            TransportFailureScope::Unattributed => Self::Unattributed,
        }
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ProxyTestError {
    #[error("proxy profile was not found")]
    ProxyNotFound,
    #[error("proxy profile is disabled")]
    ProxyDisabled,
    #[error("provider endpoint was not found")]
    ProviderEndpointNotFound,
    #[error("provider endpoint URI is invalid")]
    InvalidEndpointUri,
}
