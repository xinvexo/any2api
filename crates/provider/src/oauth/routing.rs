use any2api_domain::{ProtocolDialect, ProviderBaseUrl};

use crate::ProviderError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthRoutingProfile {
    base_url: ProviderBaseUrl,
    protocol_dialect: ProtocolDialect,
}

impl OAuthRoutingProfile {
    pub(crate) fn fixed(
        base_url: &'static str,
        protocol_dialect: ProtocolDialect,
    ) -> Result<Self, ProviderError> {
        let base_url = ProviderBaseUrl::parse(base_url).map_err(|_| {
            ProviderError::InvalidEndpoint("fixed OAuth base URL is invalid".into())
        })?;
        Ok(Self {
            base_url,
            protocol_dialect,
        })
    }

    #[must_use]
    pub const fn base_url(&self) -> &ProviderBaseUrl {
        &self.base_url
    }

    #[must_use]
    pub const fn protocol_dialect(&self) -> ProtocolDialect {
        self.protocol_dialect
    }
}
