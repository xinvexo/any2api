use super::{
    client_version as grok_client_version, headers as grok_headers, import as grok_import,
    model_catalog as grok_model_catalog, oauth as grok_oauth, quota as grok_quota,
    upstream_error as grok_error,
};
use std::sync::Arc;

use any2api_domain::{ProtocolOperation, ProviderKind, TransportMode};
use arc_swap::ArcSwapOption;
use http::HeaderMap;

use crate::{
    ProviderError, ProviderSecret,
    api::{
        CredentialHeaders, CredentialTestPlan, EndpointPlan, OAuthCapabilities,
        OAuthDeviceAuthorization, OAuthDeviceCodeProvider, OAuthDeviceTokenPoll,
        OAuthImportedAccount, OAuthLoginFlow, OAuthModelCatalogScope, OAuthQuotaProvider,
        OAuthQuotaQueryPlan, OAuthQuotaRejection, OAuthQuotaSupplement,
        OAuthQuotaSupplementProvider, OAuthQuotaUsage, OAuthRequestPlan, OAuthRoutingProfile,
        OAuthRoutingProvider, OAuthTokenMaterial, OAuthTokenProvider, OfficialClientVersion,
        OfficialClientVersionProvider, OfficialClientVersionRequestPlan, ProviderDescriptor,
        ProviderDriver, ProviderRequestContext, UpstreamResponseMeta,
    },
    credential::api_key,
};

const DESCRIPTOR: ProviderDescriptor = ProviderDescriptor::new(
    ProviderKind::Grok,
    &[
        ProtocolOperation::Responses,
        ProtocolOperation::ResponsesCompact,
        ProtocolOperation::ChatCompletions,
    ],
    Some(
        OAuthCapabilities::new(&[ProtocolOperation::Responses], OAuthLoginFlow::DeviceCode)
            .with_quota()
            .with_quota_supplement(),
    ),
    &[TransportMode::Json, TransportMode::Sse],
);

#[derive(Debug)]
pub struct GrokDriver {
    official_client_version: ArcSwapOption<OfficialClientVersion>,
}

impl Default for GrokDriver {
    fn default() -> Self {
        Self::new()
    }
}

impl GrokDriver {
    #[must_use]
    pub fn new() -> Self {
        Self {
            official_client_version: ArcSwapOption::empty(),
        }
    }

    #[must_use]
    pub fn new_with_official_client_version(version: OfficialClientVersion) -> Self {
        Self {
            official_client_version: ArcSwapOption::from(Some(Arc::new(version))),
        }
    }
}

impl ProviderDriver for GrokDriver {
    fn descriptor(&self) -> &'static ProviderDescriptor {
        &DESCRIPTOR
    }

    fn validate_credential(&self, secret: &ProviderSecret) -> Result<(), ProviderError> {
        api_key::validate_secret(secret)
    }

    fn endpoint_plan(
        &self,
        base_url: &any2api_domain::ProviderBaseUrl,
        operation: ProtocolOperation,
    ) -> Result<EndpointPlan, ProviderError> {
        if !self.descriptor().supports_api_key_operation(operation) {
            return Err(ProviderError::InvalidEndpoint(
                "operation is not supported by Grok".into(),
            ));
        }
        Ok(EndpointPlan {
            url: api_key::endpoint_url(base_url, operation)?,
        })
    }

    fn credential_test_plan(
        &self,
        base_url: &any2api_domain::ProviderBaseUrl,
    ) -> Result<CredentialTestPlan, ProviderError> {
        Ok(CredentialTestPlan {
            url: api_key::credential_test_url(base_url)?,
            headers: HeaderMap::new(),
        })
    }

    fn parse_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
        api_key::parse_model_catalog(bounded_body)
    }

    fn credential_headers(
        &self,
        _base_url: &any2api_domain::ProviderBaseUrl,
        secret: &ProviderSecret,
    ) -> Result<CredentialHeaders, ProviderError> {
        api_key::bearer_credential_headers(secret)
    }

    fn prepare_request_headers(
        &self,
        context: ProviderRequestContext<'_>,
    ) -> Result<HeaderMap, ProviderError> {
        let version = self.require_official_client_version()?;
        grok_headers::request(context, &version)
    }

    fn response_headers(&self, _operation: ProtocolOperation, upstream: &HeaderMap) -> HeaderMap {
        grok_headers::response(upstream)
    }

    fn oauth_device_code(&self) -> Option<&dyn OAuthDeviceCodeProvider> {
        Some(self)
    }

    fn oauth_token(&self) -> Option<&dyn OAuthTokenProvider> {
        Some(self)
    }

    fn oauth_routing(&self) -> Option<&dyn OAuthRoutingProvider> {
        Some(self)
    }

    fn oauth_quota(&self) -> Option<&dyn OAuthQuotaProvider> {
        Some(self)
    }

    fn official_client_version(&self) -> Option<&dyn OfficialClientVersionProvider> {
        Some(self)
    }

    fn classify_error(
        &self,
        _operation: ProtocolOperation,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> any2api_domain::UpstreamError {
        grok_error::classify(meta, bounded_body)
    }
}

impl OfficialClientVersionProvider for GrokDriver {
    fn latest_official_client_version_plan(
        &self,
    ) -> Result<OfficialClientVersionRequestPlan, ProviderError> {
        grok_client_version::request_plan()
    }

    fn parse_latest_official_client_version(
        &self,
        bounded_body: &[u8],
    ) -> Result<OfficialClientVersion, ProviderError> {
        grok_client_version::parse(bounded_body)
    }

    fn current_official_client_version(&self) -> Option<Arc<OfficialClientVersion>> {
        self.official_client_version.load_full()
    }

    fn publish_official_client_version(&self, version: OfficialClientVersion) {
        self.official_client_version.store(Some(Arc::new(version)));
    }
}

impl OAuthDeviceCodeProvider for GrokDriver {
    fn oauth_device_authorization_request(&self) -> Result<OAuthRequestPlan, ProviderError> {
        grok_oauth::device_authorization_request()
    }

    fn parse_oauth_device_authorization(
        &self,
        body: &[u8],
    ) -> Result<OAuthDeviceAuthorization, ProviderError> {
        grok_oauth::parse_device_authorization(body)
    }

    fn oauth_device_token_request(
        &self,
        device_code: &str,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        grok_oauth::device_token_request(device_code)
    }

    fn parse_oauth_device_token(
        &self,
        status: http::StatusCode,
        body: &[u8],
    ) -> Result<OAuthDeviceTokenPoll, ProviderError> {
        grok_oauth::parse_device_token(status, body)
    }
}

impl OAuthTokenProvider for GrokDriver {
    fn oauth_refresh_token_request(
        &self,
        refresh_token: &str,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        grok_oauth::refresh_request(refresh_token)
    }

    fn parse_oauth_token_response(&self, body: &[u8]) -> Result<OAuthTokenMaterial, ProviderError> {
        grok_oauth::parse_token(body)
    }

    fn parse_oauth_import(
        &self,
        object: &serde_json::Map<String, serde_json::Value>,
    ) -> Result<Option<OAuthImportedAccount>, ProviderError> {
        grok_import::parse(object)
    }
}

impl OAuthRoutingProvider for GrokDriver {
    fn oauth_routing_profile(
        &self,
        _token: &OAuthTokenMaterial,
    ) -> Result<OAuthRoutingProfile, ProviderError> {
        grok_oauth::routing_profile()
    }

    fn oauth_model_catalog_scope(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthModelCatalogScope, ProviderError> {
        grok_model_catalog::scope(token)
    }

    fn oauth_model_catalog_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<OAuthRequestPlan, ProviderError> {
        let version = self.require_official_client_version()?;
        grok_model_catalog::request_plan(token, &version)
    }

    fn parse_oauth_model_catalog(&self, bounded_body: &[u8]) -> Result<Vec<String>, ProviderError> {
        grok_model_catalog::parse(bounded_body)
    }

    fn oauth_credential_headers(
        &self,
        token: &OAuthTokenMaterial,
        _forwarded: &HeaderMap,
    ) -> Result<CredentialHeaders, ProviderError> {
        grok_oauth::credential_headers(token)
    }
}

impl OAuthQuotaProvider for GrokDriver {
    fn oauth_quota_query_plan(
        &self,
        token: &OAuthTokenMaterial,
    ) -> Result<Option<OAuthQuotaQueryPlan>, ProviderError> {
        let version = self.require_official_client_version()?;
        grok_quota::query_plan(token, &version).map(Some)
    }

    fn classify_oauth_quota_rejection(
        &self,
        meta: &UpstreamResponseMeta,
        bounded_body: &[u8],
    ) -> OAuthQuotaRejection {
        grok_error::classify_oauth_quota_rejection(meta, bounded_body)
    }

    fn parse_oauth_quota_usage(
        &self,
        _meta: &UpstreamResponseMeta,
        body: &[u8],
    ) -> Result<OAuthQuotaUsage, ProviderError> {
        grok_quota::parse_usage(body)
    }

    fn supplement(&self) -> Option<&dyn OAuthQuotaSupplementProvider> {
        Some(self)
    }
}

impl OAuthQuotaSupplementProvider for GrokDriver {
    fn parse_oauth_quota_supplement(
        &self,
        body: &[u8],
    ) -> Result<OAuthQuotaSupplement, ProviderError> {
        grok_quota::parse_subscription(body)
    }
}
