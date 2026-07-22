use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use any2api_domain::{ConfigRevision, CredentialId, ProviderEndpointId, ProxyProfileId};
use any2api_provider::api::{ProviderError, ProviderRegistry};
use any2api_transport::api::{
    EndpointNetworkPolicy, TransportErrorStage, TransportFailureScope, TransportManager,
    TransportRequest,
};
use bytes::Bytes;
use http::Method;
use thiserror::Error;

use crate::{
    provider_model_catalog::{ModelCatalogReadError, collect as collect_model_catalog},
    published_snapshot::PublishedSnapshot,
};

pub struct ProviderCredentialTestService {
    providers: Arc<ProviderRegistry>,
    transport: Arc<dyn TransportManager>,
}

impl ProviderCredentialTestService {
    #[must_use]
    pub fn new(providers: Arc<ProviderRegistry>, transport: Arc<dyn TransportManager>) -> Self {
        Self {
            providers,
            transport,
        }
    }

    pub async fn test(
        &self,
        snapshot: Arc<PublishedSnapshot>,
        credential_id: CredentialId,
    ) -> Result<ProviderCredentialTestResult, ProviderCredentialTestError> {
        let credential = snapshot
            .provider_credentials()
            .get(credential_id)
            .ok_or(ProviderCredentialTestError::CredentialNotFound)?;
        if !credential.enabled() {
            return Err(ProviderCredentialTestError::CredentialDisabled);
        }
        let endpoint = snapshot
            .provider_endpoints()
            .get(credential.provider_endpoint_id())
            .ok_or(ProviderCredentialTestError::ProviderEndpointNotFound)?;
        if !endpoint.enabled() {
            return Err(ProviderCredentialTestError::ProviderEndpointDisabled);
        }
        let binding = snapshot
            .credential_runtime(credential_id)
            .ok_or(ProviderCredentialTestError::CredentialRuntimeUnavailable)?;
        let proxy = snapshot
            .resolved_transport_proxy_for_credential(credential_id)
            .ok_or(ProviderCredentialTestError::ProxyNotFound)?;
        if !proxy.profile().enabled() {
            return Err(ProviderCredentialTestError::ProxyDisabled);
        }
        let driver = self
            .providers
            .get(endpoint.provider_kind())
            .ok_or(ProviderCredentialTestError::ProviderUnavailable)?;
        let endpoint_plan = driver
            .credential_test_plan(endpoint.base_url())
            .map_err(ProviderCredentialTestError::Provider)?;
        let permit = binding
            .try_acquire()
            .ok_or(ProviderCredentialTestError::CredentialAtCapacity)?;
        let credential_headers = permit
            .provider_credential_headers(driver.as_ref())
            .map_err(ProviderCredentialTestError::Provider)?;
        let request = TransportRequest {
            method: Method::GET,
            uri: endpoint_plan
                .url
                .as_str()
                .parse()
                .map_err(|_| ProviderCredentialTestError::InvalidEndpointUri)?,
            headers: credential_headers.headers,
            body: Bytes::new(),
            network_policy: EndpointNetworkPolicy::new(endpoint.allow_private_network())
                .with_strict_ssrf(snapshot.settings().upstream().strict_ssrf()),
            read_timeout: std::time::Duration::from_millis(
                snapshot.settings().upstream().read_timeout_ms(),
            ),
        };
        let captured = CapturedTestConfiguration {
            config_revision: snapshot.revision(),
            provider_endpoint_config_version: endpoint.config_version(),
            credential_config_version: credential.config_version(),
            credential_generation: credential.credential_generation(),
            secret_version: credential.secret_version(),
            proxy_config_version: proxy.profile().config_version(),
            credential_id,
            provider_endpoint_id: endpoint.id(),
            proxy_id: proxy.profile().id(),
        };
        let started = Instant::now();
        let outcome = match self.transport.execute(proxy, request).await {
            Ok(response) if response.status.is_success() => {
                let status_code = response.status.as_u16();
                let read_timeout =
                    Duration::from_millis(snapshot.settings().upstream().read_timeout_ms());
                match collect_model_catalog(
                    response.body,
                    read_timeout,
                    response.read_failure_scope,
                )
                .await
                {
                    Ok(body) => match driver.parse_model_catalog(&body) {
                        Ok(models) => ProviderCredentialTestOutcome::Accepted {
                            status_code,
                            auth_error_cleared: permit.generation().health().clear_auth_error(),
                            models,
                        },
                        Err(_) => ProviderCredentialTestOutcome::InvalidCatalog { status_code },
                    },
                    Err(ModelCatalogReadError::Transport(error)) => {
                        ProviderCredentialTestOutcome::Failed {
                            stage: error.stage.into(),
                            scope: error.failure_scope.into(),
                        }
                    }
                    Err(ModelCatalogReadError::TooLarge) => {
                        ProviderCredentialTestOutcome::InvalidCatalog { status_code }
                    }
                }
            }
            Ok(response) => ProviderCredentialTestOutcome::Rejected {
                status_code: response.status.as_u16(),
            },
            Err(error) => ProviderCredentialTestOutcome::Failed {
                stage: error.stage.into(),
                scope: error.failure_scope.into(),
            },
        };
        Ok(captured.finish(started, outcome))
    }
}

struct CapturedTestConfiguration {
    config_revision: ConfigRevision,
    provider_endpoint_config_version: u64,
    credential_config_version: u64,
    credential_generation: u64,
    secret_version: u64,
    proxy_config_version: u64,
    credential_id: CredentialId,
    provider_endpoint_id: ProviderEndpointId,
    proxy_id: ProxyProfileId,
}

impl CapturedTestConfiguration {
    fn finish(
        self,
        started: Instant,
        outcome: ProviderCredentialTestOutcome,
    ) -> ProviderCredentialTestResult {
        ProviderCredentialTestResult {
            config_revision: self.config_revision,
            provider_endpoint_config_version: self.provider_endpoint_config_version,
            credential_config_version: self.credential_config_version,
            credential_generation: self.credential_generation,
            secret_version: self.secret_version,
            proxy_config_version: self.proxy_config_version,
            credential_id: self.credential_id,
            provider_endpoint_id: self.provider_endpoint_id,
            proxy_id: self.proxy_id,
            latency_ms: u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX),
            outcome,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderCredentialTestResult {
    pub config_revision: ConfigRevision,
    pub provider_endpoint_config_version: u64,
    pub credential_config_version: u64,
    pub credential_generation: u64,
    pub secret_version: u64,
    pub proxy_config_version: u64,
    pub credential_id: CredentialId,
    pub provider_endpoint_id: ProviderEndpointId,
    pub proxy_id: ProxyProfileId,
    pub latency_ms: u64,
    pub outcome: ProviderCredentialTestOutcome,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderCredentialTestOutcome {
    Accepted {
        status_code: u16,
        auth_error_cleared: bool,
        models: Vec<String>,
    },
    InvalidCatalog {
        status_code: u16,
    },
    Rejected {
        status_code: u16,
    },
    Failed {
        stage: ProviderCredentialTestFailureStage,
        scope: ProviderCredentialTestFailureScope,
    },
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ProviderCredentialTestFailureStage {
    Dns,
    Tcp,
    ProxyHandshake,
    Tls,
    WriteRequest,
    AwaitHeaders,
    ReadBody,
}

impl ProviderCredentialTestFailureStage {
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

impl From<TransportErrorStage> for ProviderCredentialTestFailureStage {
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
pub enum ProviderCredentialTestFailureScope {
    Endpoint,
    Proxy,
    Unattributed,
}

impl ProviderCredentialTestFailureScope {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Endpoint => "endpoint",
            Self::Proxy => "proxy",
            Self::Unattributed => "unattributed",
        }
    }
}

impl From<TransportFailureScope> for ProviderCredentialTestFailureScope {
    fn from(value: TransportFailureScope) -> Self {
        match value {
            TransportFailureScope::Endpoint => Self::Endpoint,
            TransportFailureScope::Proxy => Self::Proxy,
            TransportFailureScope::Unattributed => Self::Unattributed,
        }
    }
}

#[derive(Debug, Error)]
pub enum ProviderCredentialTestError {
    #[error("provider credential was not found")]
    CredentialNotFound,
    #[error("provider credential is disabled")]
    CredentialDisabled,
    #[error("provider endpoint was not found")]
    ProviderEndpointNotFound,
    #[error("provider endpoint is disabled")]
    ProviderEndpointDisabled,
    #[error("credential runtime is unavailable")]
    CredentialRuntimeUnavailable,
    #[error("resolved proxy was not found")]
    ProxyNotFound,
    #[error("resolved proxy is disabled")]
    ProxyDisabled,
    #[error("provider driver is unavailable")]
    ProviderUnavailable,
    #[error("provider credential is at capacity")]
    CredentialAtCapacity,
    #[error("provider endpoint URI is invalid")]
    InvalidEndpointUri,
    #[error("provider test request is invalid: {0}")]
    Provider(ProviderError),
}
