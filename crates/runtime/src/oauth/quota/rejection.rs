//! Evidence-based OAuth quota rejection mapping and provider egress probing.

use std::{
    collections::HashMap,
    future::Future,
    sync::Arc,
    time::{Duration, Instant},
};

use any2api_domain::{ConfigRevision, ProviderKind};
use any2api_provider::api::{
    OAuthProviderEgressStatus, OAuthQuotaRejection, OAuthRequestPlan, ProviderDriver,
    UpstreamResponseMeta,
};
use any2api_transport::api::{TransportManager, TransportProxy};
use http::StatusCode;
use tokio::sync::{Mutex, OnceCell};

use super::{
    request::{self, OAuthQuotaResponse},
    types::OAuthQuotaError,
};

const EGRESS_PROBE_CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Default)]
pub(super) struct EgressProbeCache {
    entries: Mutex<HashMap<ProbeKey, Arc<OnceCell<CachedProbe>>>>,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
struct ProbeKey {
    provider: ProviderKind,
    revision: u64,
}

#[derive(Clone, Copy)]
struct CachedProbe {
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
        let key = ProbeKey {
            provider: resolver.driver.kind(),
            revision: resolver.revision.get(),
        };
        self.resolve(key, || resolver.execute_probe(plan)).await
    }

    async fn resolve<F, Fut>(&self, key: ProbeKey, probe: F) -> OAuthProviderEgressStatus
    where
        F: FnOnce() -> Fut,
        Fut: Future<Output = OAuthProviderEgressStatus>,
    {
        let slot = {
            let mut entries = self.entries.lock().await;
            entries.retain(|_, slot| {
                slot.get()
                    .is_none_or(|cached| cached.observed_at.elapsed() < EGRESS_PROBE_CACHE_TTL)
            });
            Arc::clone(
                entries
                    .entry(key)
                    .or_insert_with(|| Arc::new(OnceCell::new())),
            )
        };
        slot.get_or_init(|| async move {
            let status = probe().await;
            CachedProbe {
                observed_at: Instant::now(),
                status,
            }
        })
        .await
        .status
    }
}

#[cfg(test)]
#[path = "rejection_tests.rs"]
mod tests;
