//! Evidence-based OAuth quota rejection mapping and provider egress probing.

use std::{
    collections::HashMap,
    time::{Duration, Instant},
};

use any2api_domain::{ConfigRevision, ProviderKind};
use any2api_provider::api::{
    OAuthProviderEgressStatus, OAuthQuotaRejection, OAuthRequestPlan, ProviderDriver,
    UpstreamResponseMeta,
};
use any2api_transport::api::{TransportManager, TransportProxy};
use http::StatusCode;
use tokio::sync::Mutex;

use super::{
    request::{self, OAuthQuotaResponse},
    types::OAuthQuotaError,
};

const EGRESS_PROBE_CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(super) struct EgressProbeCache {
    entries: Mutex<HashMap<ProviderKind, CachedProbe>>,
}

#[derive(Clone, Copy)]
struct CachedProbe {
    revision: ConfigRevision,
    observed_at: Instant,
    status: OAuthProviderEgressStatus,
}

pub(super) struct RequestContext<'a> {
    driver: &'a dyn ProviderDriver,
    transport: &'a dyn TransportManager,
    proxy: TransportProxy<'a>,
    strict_ssrf: bool,
    read_timeout: Duration,
    revision: ConfigRevision,
    probes: &'a EgressProbeCache,
}

impl<'a> RequestContext<'a> {
    pub(super) const fn new(
        driver: &'a dyn ProviderDriver,
        transport: &'a dyn TransportManager,
        proxy: TransportProxy<'a>,
        strict_ssrf: bool,
        read_timeout: Duration,
        revision: ConfigRevision,
        probes: &'a EgressProbeCache,
    ) -> Self {
        Self {
            driver,
            transport,
            proxy,
            strict_ssrf,
            read_timeout,
            revision,
            probes,
        }
    }

    pub(super) const fn driver(&self) -> &'a dyn ProviderDriver {
        self.driver
    }

    pub(super) async fn execute(
        &self,
        plan: OAuthRequestPlan,
    ) -> Result<OAuthQuotaResponse, OAuthQuotaError> {
        request::execute(
            self.transport,
            self.proxy,
            self.strict_ssrf,
            self.read_timeout,
            plan,
        )
        .await
    }

    pub(super) async fn rejection(&self, response: &OAuthQuotaResponse) -> OAuthQuotaError {
        if response.status == StatusCode::UNAUTHORIZED {
            return OAuthQuotaError::UpstreamRejected(response.status.as_u16());
        }
        let meta = UpstreamResponseMeta {
            status: response.status,
            headers: response.headers.clone(),
        };
        match self
            .driver
            .classify_oauth_quota_rejection(&meta, &response.body)
        {
            OAuthQuotaRejection::AccountRestricted => OAuthQuotaError::AccountRestricted,
            OAuthQuotaRejection::ProviderEgressRestricted => {
                OAuthQuotaError::ProviderEgressRestricted
            }
            OAuthQuotaRejection::Unclassified if response.status == StatusCode::FORBIDDEN => {
                let plan = match self.driver.oauth_provider_egress_probe_plan() {
                    Ok(Some(plan)) => plan,
                    Ok(None) | Err(_) => {
                        return OAuthQuotaError::UpstreamRejected(response.status.as_u16());
                    }
                };
                if self.probes.inspect(self, plan).await == OAuthProviderEgressStatus::Restricted {
                    OAuthQuotaError::ProviderEgressRestricted
                } else {
                    OAuthQuotaError::UpstreamRejected(response.status.as_u16())
                }
            }
            OAuthQuotaRejection::Unclassified => {
                OAuthQuotaError::UpstreamRejected(response.status.as_u16())
            }
        }
    }

    async fn execute_probe(&self, plan: OAuthRequestPlan) -> OAuthProviderEgressStatus {
        let response = match self.execute(plan).await {
            Ok(response) => response,
            Err(_) => return OAuthProviderEgressStatus::Unverified,
        };
        self.driver.classify_oauth_provider_egress(
            &UpstreamResponseMeta {
                status: response.status,
                headers: response.headers,
            },
            &response.body,
        )
    }
}

impl EgressProbeCache {
    async fn inspect(
        &self,
        resolver: &RequestContext<'_>,
        plan: OAuthRequestPlan,
    ) -> OAuthProviderEgressStatus {
        let provider = resolver.driver.kind();
        let mut entries = self.entries.lock().await;
        if let Some(cached) = entries.get(&provider)
            && cached.revision == resolver.revision
            && cached.observed_at.elapsed() < EGRESS_PROBE_CACHE_TTL
        {
            return cached.status;
        }
        let status = resolver.execute_probe(plan).await;
        entries.insert(
            provider,
            CachedProbe {
                revision: resolver.revision,
                observed_at: Instant::now(),
                status,
            },
        );
        status
    }
}
