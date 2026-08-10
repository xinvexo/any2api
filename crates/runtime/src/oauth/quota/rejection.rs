//! Evidence-based OAuth quota rejection mapping from the original response.

use std::time::Duration;

use any2api_provider::api::{
    OAuthQuotaRejection, OAuthRequestPlan, ProviderDriver, UpstreamResponseMeta,
};
use any2api_transport::api::{TransportIsolationKey, TransportManager, TransportProxy};
use http::StatusCode;

use super::{
    request::{self, OAuthQuotaResponse},
    types::OAuthQuotaError,
};

pub(super) struct RequestContext<'a> {
    driver: &'a dyn ProviderDriver,
    transport: &'a dyn TransportManager,
    proxy: TransportProxy<'a>,
    strict_ssrf: bool,
    read_timeout: Duration,
    isolation: TransportIsolationKey,
}

impl<'a> RequestContext<'a> {
    pub(super) const fn new(
        driver: &'a dyn ProviderDriver,
        transport: &'a dyn TransportManager,
        proxy: TransportProxy<'a>,
        strict_ssrf: bool,
        read_timeout: Duration,
        isolation: TransportIsolationKey,
    ) -> Self {
        Self {
            driver,
            transport,
            proxy,
            strict_ssrf,
            read_timeout,
            isolation,
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
            self.isolation,
            plan,
        )
        .await
    }

    pub(super) fn rejection(&self, response: &OAuthQuotaResponse) -> OAuthQuotaError {
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
            OAuthQuotaRejection::Unclassified => {
                OAuthQuotaError::UpstreamRejected(response.status.as_u16())
            }
        }
    }
}
