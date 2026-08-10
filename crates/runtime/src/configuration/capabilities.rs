use std::sync::Arc;

use any2api_domain::{
    CredentialKind, ModelRouteConfiguration, ProtocolDialect, ProtocolOperation,
    ProviderCredentialConfiguration, ProviderEndpointConfiguration, ProviderEndpointId,
    ProviderKind,
};
use any2api_protocol::api::ProtocolRegistry;
use any2api_provider::api::ProviderRegistry;
use thiserror::Error;

#[derive(Clone)]
pub struct ConfigurationCapabilities {
    protocols: Arc<ProtocolRegistry>,
    providers: Arc<ProviderRegistry>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderProtocolOptions {
    pub provider_kind: ProviderKind,
    pub accepted_protocol: ProtocolDialect,
    pub upstream_protocols: Vec<ProtocolDialect>,
}

#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
pub enum ConfigurationCapabilityError {
    #[error("provider endpoint is not present in the candidate configuration: {0}")]
    MissingProviderEndpoint(ProviderEndpointId),
    #[error("provider driver is not registered: {0:?}")]
    MissingProviderDriver(ProviderKind),
    #[error("protocol adapter is not registered: {0:?}")]
    MissingProtocolAdapter(ProtocolDialect),
    #[error("protocol bridge is not registered: {ingress:?} -> {upstream:?}")]
    MissingProtocolBridge {
        ingress: ProtocolDialect,
        upstream: ProtocolDialect,
    },
    #[error("provider {provider:?} does not support upstream protocol {protocol:?}")]
    UnsupportedProviderProtocol {
        provider: ProviderKind,
        protocol: ProtocolDialect,
    },
    #[error("provider {provider:?} does not support credential kind {credential:?}")]
    UnsupportedCredentialKind {
        provider: ProviderKind,
        credential: CredentialKind,
    },
}

impl ConfigurationCapabilities {
    #[must_use]
    pub fn new(protocols: Arc<ProtocolRegistry>, providers: Arc<ProviderRegistry>) -> Self {
        Self {
            protocols,
            providers,
        }
    }

    #[must_use]
    pub fn protocol_registry(&self) -> &ProtocolRegistry {
        self.protocols.as_ref()
    }

    #[must_use]
    pub fn provider_registry(&self) -> &ProviderRegistry {
        self.providers.as_ref()
    }

    pub fn validate_endpoint(
        &self,
        provider: ProviderKind,
        accepted: ProtocolDialect,
        upstream: ProtocolDialect,
    ) -> Result<(), ConfigurationCapabilityError> {
        self.validate_protocol_pair(accepted, upstream)?;
        let driver = self.providers.get(provider).ok_or(
            ConfigurationCapabilityError::MissingProviderDriver(provider),
        )?;
        if !driver.capabilities().protocols.contains(&upstream) {
            return Err(ConfigurationCapabilityError::UnsupportedProviderProtocol {
                provider,
                protocol: upstream,
            });
        }
        Ok(())
    }

    pub fn validate_credential(
        &self,
        provider: ProviderKind,
        credential: CredentialKind,
    ) -> Result<(), ConfigurationCapabilityError> {
        let driver = self.providers.get(provider).ok_or(
            ConfigurationCapabilityError::MissingProviderDriver(provider),
        )?;
        if !driver.capabilities().credential_kinds.contains(&credential) {
            return Err(ConfigurationCapabilityError::UnsupportedCredentialKind {
                provider,
                credential,
            });
        }
        Ok(())
    }

    pub fn validate_configuration(
        &self,
        endpoints: &ProviderEndpointConfiguration,
        credentials: &ProviderCredentialConfiguration,
        routes: &ModelRouteConfiguration,
    ) -> Result<(), ConfigurationCapabilityError> {
        for endpoint in endpoints.endpoints() {
            self.validate_endpoint(
                endpoint.provider_kind(),
                endpoint.protocol_dialect(),
                endpoint.effective_upstream_protocol_dialect(),
            )?;
        }
        for credential in credentials.credentials() {
            let endpoint = endpoints.get(credential.provider_endpoint_id()).ok_or(
                ConfigurationCapabilityError::MissingProviderEndpoint(
                    credential.provider_endpoint_id(),
                ),
            )?;
            self.validate_credential(endpoint.provider_kind(), credential.credential_kind())?;
        }
        for route in routes.routes() {
            for target in route.targets() {
                let endpoint = endpoints.get(target.provider_endpoint_id()).ok_or(
                    ConfigurationCapabilityError::MissingProviderEndpoint(
                        target.provider_endpoint_id(),
                    ),
                )?;
                self.validate_endpoint(
                    endpoint.provider_kind(),
                    route.ingress_protocol(),
                    target.upstream_protocol_dialect(),
                )?;
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn provider_protocol_options(
        &self,
        provider: ProviderKind,
    ) -> Vec<ProviderProtocolOptions> {
        let Some(driver) = self.providers.get(provider) else {
            return Vec::new();
        };
        let mut accepted = self
            .protocols
            .iter()
            .map(|(dialect, _)| *dialect)
            .filter(|dialect| has_operation(*dialect))
            .collect::<Vec<_>>();
        accepted.sort_unstable();
        accepted
            .into_iter()
            .filter_map(|accepted_protocol| {
                let mut upstream_protocols = driver
                    .capabilities()
                    .protocols
                    .iter()
                    .copied()
                    .filter(|upstream| self.protocols.supports_pair(accepted_protocol, *upstream))
                    .collect::<Vec<_>>();
                upstream_protocols.sort_unstable();
                (!upstream_protocols.is_empty()).then_some(ProviderProtocolOptions {
                    provider_kind: provider,
                    accepted_protocol,
                    upstream_protocols,
                })
            })
            .collect()
    }

    fn validate_protocol_pair(
        &self,
        ingress: ProtocolDialect,
        upstream: ProtocolDialect,
    ) -> Result<(), ConfigurationCapabilityError> {
        if self.protocols.get(ingress).is_none() {
            return Err(ConfigurationCapabilityError::MissingProtocolAdapter(
                ingress,
            ));
        }
        if self.protocols.get(upstream).is_none() {
            return Err(ConfigurationCapabilityError::MissingProtocolAdapter(
                upstream,
            ));
        }
        if !has_operation(ingress) {
            return Err(ConfigurationCapabilityError::MissingProtocolAdapter(
                ingress,
            ));
        }
        if ingress != upstream
            && !ProtocolOperation::ALL.iter().copied().any(|operation| {
                self.protocols
                    .supports_operation(ingress, upstream, operation)
            })
        {
            return Err(ConfigurationCapabilityError::MissingProtocolBridge { ingress, upstream });
        }
        Ok(())
    }
}

fn has_operation(dialect: ProtocolDialect) -> bool {
    ProtocolOperation::ALL
        .iter()
        .copied()
        .any(|operation| operation.dialect() == dialect)
}

#[cfg(test)]
mod tests {
    use any2api_domain::{ProtocolDialect, ProviderKind};

    use super::ConfigurationCapabilityError;

    #[test]
    fn options_are_derived_from_registered_bridges_and_provider_capabilities() {
        let capabilities = crate::test_support::configuration_capabilities();
        let codex = capabilities.provider_protocol_options(ProviderKind::Codex);

        assert_eq!(codex.len(), 3);
        assert_eq!(codex[0].accepted_protocol, ProtocolDialect::OpenAiResponses);
        assert_eq!(
            codex[0].upstream_protocols,
            [
                ProtocolDialect::OpenAiResponses,
                ProtocolDialect::OpenAiChatCompletions,
            ]
        );
        assert_eq!(
            codex[1].upstream_protocols,
            [ProtocolDialect::OpenAiChatCompletions]
        );
        assert_eq!(codex[2].accepted_protocol, ProtocolDialect::OpenAiImages);
        assert_eq!(
            codex[2].upstream_protocols,
            [
                ProtocolDialect::OpenAiChatCompletions,
                ProtocolDialect::OpenAiImages,
            ]
        );

        let grok = capabilities.provider_protocol_options(ProviderKind::Grok);
        assert_eq!(grok.len(), 3);
        assert_eq!(grok[0].accepted_protocol, ProtocolDialect::OpenAiResponses);
        assert_eq!(
            grok[0].upstream_protocols,
            [
                ProtocolDialect::OpenAiResponses,
                ProtocolDialect::OpenAiChatCompletions,
            ]
        );
        assert_eq!(grok[2].accepted_protocol, ProtocolDialect::OpenAiImages);
        assert_eq!(
            grok[2].upstream_protocols,
            [ProtocolDialect::OpenAiChatCompletions]
        );

        let kimi = capabilities.provider_protocol_options(ProviderKind::Kimi);
        assert_eq!(kimi.len(), 3);
        assert_eq!(kimi[0].accepted_protocol, ProtocolDialect::OpenAiResponses);
        assert_eq!(
            kimi[0].upstream_protocols,
            [ProtocolDialect::OpenAiChatCompletions]
        );
        assert_eq!(
            kimi[1].upstream_protocols,
            [ProtocolDialect::OpenAiChatCompletions]
        );
    }

    #[test]
    fn endpoint_validation_uses_the_registered_pair_and_provider_driver() {
        let capabilities = crate::test_support::configuration_capabilities();
        capabilities
            .validate_endpoint(
                ProviderKind::Codex,
                ProtocolDialect::OpenAiResponses,
                ProtocolDialect::OpenAiChatCompletions,
            )
            .expect("registered bridge");
        capabilities
            .validate_endpoint(
                ProviderKind::Codex,
                ProtocolDialect::OpenAiImages,
                ProtocolDialect::OpenAiImages,
            )
            .expect("registered Images adapter and Codex capability");
        capabilities
            .validate_endpoint(
                ProviderKind::Codex,
                ProtocolDialect::OpenAiImages,
                ProtocolDialect::OpenAiChatCompletions,
            )
            .expect("registered Images to Chat Completions bridge");
        capabilities
            .validate_endpoint(
                ProviderKind::Grok,
                ProtocolDialect::OpenAiImages,
                ProtocolDialect::OpenAiChatCompletions,
            )
            .expect("bridge options are derived without a Provider-specific branch");
        capabilities
            .validate_endpoint(
                ProviderKind::Kimi,
                ProtocolDialect::OpenAiResponses,
                ProtocolDialect::OpenAiChatCompletions,
            )
            .expect("Kimi uses the registered Responses to Chat bridge");

        assert!(matches!(
            capabilities.validate_endpoint(
                ProviderKind::Codex,
                ProtocolDialect::AnthropicMessages,
                ProtocolDialect::OpenAiResponses,
            ),
            Err(ConfigurationCapabilityError::MissingProtocolBridge { .. })
        ));
        assert!(matches!(
            capabilities.validate_endpoint(
                ProviderKind::Claude,
                ProtocolDialect::OpenAiResponses,
                ProtocolDialect::OpenAiResponses,
            ),
            Err(ConfigurationCapabilityError::UnsupportedProviderProtocol { .. })
        ));
        assert!(matches!(
            capabilities.validate_endpoint(
                ProviderKind::Grok,
                ProtocolDialect::OpenAiImages,
                ProtocolDialect::OpenAiImages,
            ),
            Err(ConfigurationCapabilityError::UnsupportedProviderProtocol { .. })
        ));
        assert!(matches!(
            capabilities.validate_endpoint(
                ProviderKind::Kimi,
                ProtocolDialect::OpenAiResponses,
                ProtocolDialect::OpenAiResponses,
            ),
            Err(ConfigurationCapabilityError::UnsupportedProviderProtocol { .. })
        ));
    }
}
