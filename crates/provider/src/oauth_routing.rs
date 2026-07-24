use any2api_domain::{ProtocolDialect, ProviderBaseUrl, UpstreamModelName};

use crate::ProviderError;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthRoutingProfile {
    base_url: ProviderBaseUrl,
    protocol_dialect: ProtocolDialect,
    models: Vec<UpstreamModelName>,
}

impl OAuthRoutingProfile {
    pub(crate) fn fixed(
        base_url: &'static str,
        protocol_dialect: ProtocolDialect,
        models: &'static [&'static str],
    ) -> Result<Self, ProviderError> {
        let base_url = ProviderBaseUrl::parse(base_url).map_err(|_| {
            ProviderError::InvalidEndpoint("fixed OAuth base URL is invalid".into())
        })?;
        let mut models = models
            .iter()
            .map(|model| {
                UpstreamModelName::new((*model).to_owned()).map_err(|_| {
                    ProviderError::InvalidResponse("fixed OAuth model catalog is invalid".into())
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        models.sort();
        if models.is_empty() || models.windows(2).any(|pair| pair[0] == pair[1]) {
            return Err(ProviderError::InvalidResponse(
                "fixed OAuth model catalog is invalid".into(),
            ));
        }
        Ok(Self {
            base_url,
            protocol_dialect,
            models,
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

    #[must_use]
    pub fn models(&self) -> &[UpstreamModelName] {
        &self.models
    }
}
