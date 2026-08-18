use std::{collections::HashMap, sync::Arc};

use any2api_domain::{
    OAuthAccount, OAuthAccountConfiguration, OAuthAccountId, ProviderCredentialConfiguration,
    ProviderEndpointConfiguration, ProviderEndpointId, ProviderKind, ProxyConfiguration,
};
use any2api_provider::api::{ProviderRegistry, decode_oauth_account_document};
use any2api_storage::api::{SecretBytes, StoredOAuthAccountMaterial, StoredOAuthAccountMaterials};
use secrecy::ExposeSecret;
use thiserror::Error;
use uuid::Uuid;

use super::{RoutingCredentialSpec, projection::RoutingCredentialProjection};
use crate::credential::{
    CredentialAuthMaterialError, CredentialAuthMaterials, CredentialAuthentication,
    CredentialGenerationDefinition,
};

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub(crate) enum RoutingCredentialCompileError {
    #[error(transparent)]
    ProviderApiKey(#[from] CredentialAuthMaterialError),
    #[error("provider endpoint is missing for credential {0}")]
    MissingProviderEndpoint(any2api_domain::CredentialId),
    #[error("resolved proxy is missing for credential {0}")]
    MissingCredentialProxy(any2api_domain::CredentialId),
    #[error("provider driver is not registered: {0:?}")]
    MissingProviderDriver(ProviderKind),
    #[error("duplicate OAuth material for account {0}")]
    DuplicateOAuthMaterial(OAuthAccountId),
    #[error("OAuth material is missing for account {0}")]
    MissingOAuthMaterial(OAuthAccountId),
    #[error("OAuth material Provider does not match account {0}")]
    OAuthProviderMismatch(OAuthAccountId),
    #[error("OAuth material version does not match account {0}")]
    OAuthVersionMismatch(OAuthAccountId),
    #[error("OAuth material generation does not match account {0}")]
    OAuthGenerationMismatch(OAuthAccountId),
    #[error("OAuth document is invalid for account {0}")]
    InvalidOAuthDocument(OAuthAccountId),
    #[error("OAuth routing profile is invalid for account {0}")]
    InvalidOAuthRoutingProfile(OAuthAccountId),
    #[error("the global proxy could not be resolved for OAuth account {0}")]
    MissingOAuthProxy(OAuthAccountId),
    #[error("OAuth material references unknown account {0}")]
    UnknownOAuthMaterial(OAuthAccountId),
}

impl RoutingCredentialSpec {
    pub(crate) fn compile(
        credentials: &ProviderCredentialConfiguration,
        endpoints: &ProviderEndpointConfiguration,
        oauth_accounts: &OAuthAccountConfiguration,
        proxies: &ProxyConfiguration,
        mut credential_auth: CredentialAuthMaterials,
        oauth_materials: StoredOAuthAccountMaterials,
        providers: &ProviderRegistry,
    ) -> Result<Vec<Self>, RoutingCredentialCompileError> {
        let mut specs =
            Vec::with_capacity(credentials.credentials().len() + oauth_accounts.accounts().len());
        compile_provider_credentials(
            &mut specs,
            credentials,
            endpoints,
            proxies,
            &mut credential_auth,
            providers,
        )?;
        credential_auth.ensure_consumed()?;
        compile_oauth_accounts(
            &mut specs,
            oauth_accounts,
            proxies,
            oauth_materials,
            providers,
        )?;
        Ok(specs)
    }
}

fn compile_provider_credentials(
    specs: &mut Vec<RoutingCredentialSpec>,
    credentials: &ProviderCredentialConfiguration,
    endpoints: &ProviderEndpointConfiguration,
    proxies: &ProxyConfiguration,
    credential_auth: &mut CredentialAuthMaterials,
    providers: &ProviderRegistry,
) -> Result<(), RoutingCredentialCompileError> {
    for credential in credentials.credentials() {
        let endpoint = endpoints.get(credential.provider_endpoint_id()).ok_or(
            RoutingCredentialCompileError::MissingProviderEndpoint(credential.id()),
        )?;
        if providers.get(endpoint.provider_kind()).is_none() {
            return Err(RoutingCredentialCompileError::MissingProviderDriver(
                endpoint.provider_kind(),
            ));
        }
        let proxy = proxies.get(credential.proxy_profile_id()).ok_or(
            RoutingCredentialCompileError::MissingCredentialProxy(credential.id()),
        )?;
        let auth = credential_auth.take_for(credential)?;
        let upstream_models = credential
            .models()
            .iter()
            .map(|model| model.upstream_model().clone())
            .collect::<Vec<_>>();
        specs.push(RoutingCredentialSpec {
            projection: RoutingCredentialProjection {
                id: credential.id().into(),
                provider_kind: endpoint.provider_kind(),
                endpoint_id: endpoint.id(),
                endpoint_config_version: endpoint.config_version(),
                base_url: endpoint.base_url().clone(),
                ingress_protocol: endpoint.protocol_dialect(),
                upstream_protocol: endpoint.effective_upstream_protocol_dialect(),
                proxy_id: proxy.id(),
                proxy_config_version: proxy.config_version(),
                enabled: credential.enabled(),
                expires_at: None,
                endpoint_enabled: endpoint.enabled(),
                proxy_enabled: proxy.enabled(),
                models: upstream_models.clone(),
                available_models: upstream_models,
            },
            requests_per_minute: credential.requests_per_minute(),
            generation: CredentialGenerationDefinition::new(
                credential.credential_generation(),
                credential.secret_version(),
                CredentialAuthentication::provider_api_key(auth.into_provider_secret()),
            ),
        });
    }
    Ok(())
}

fn compile_oauth_accounts(
    specs: &mut Vec<RoutingCredentialSpec>,
    accounts: &OAuthAccountConfiguration,
    proxies: &ProxyConfiguration,
    materials: StoredOAuthAccountMaterials,
    providers: &ProviderRegistry,
) -> Result<(), RoutingCredentialCompileError> {
    let mut materials = OAuthMaterials::new(materials)?;
    for account in accounts.accounts() {
        let driver = providers.get(account.provider_kind()).ok_or(
            RoutingCredentialCompileError::MissingProviderDriver(account.provider_kind()),
        )?;
        let material = materials.take_for(account)?;
        let token = decode_oauth_account_document(
            account.provider_kind(),
            account.expires_at(),
            material.expose_secret(),
        )
        .map_err(|_| RoutingCredentialCompileError::InvalidOAuthDocument(account.id()))?;
        let profile = driver
            .oauth_routing_profile(&token)
            .map_err(|_| RoutingCredentialCompileError::InvalidOAuthRoutingProfile(account.id()))?;
        let proxy = proxies.resolve_oauth(account.proxy_selection()).ok_or(
            RoutingCredentialCompileError::MissingOAuthProxy(account.id()),
        )?;
        // OAuth model directories are fetched from upstream and persisted outside
        // the published routing snapshot. Only explicitly saved selections route.
        let available_models = account.models().to_vec();
        specs.push(RoutingCredentialSpec {
            projection: RoutingCredentialProjection {
                id: account.id().into(),
                provider_kind: account.provider_kind(),
                endpoint_id: oauth_endpoint_id(account.provider_kind()),
                endpoint_config_version: 1,
                base_url: profile.base_url().clone(),
                ingress_protocol: profile.protocol_dialect(),
                upstream_protocol: profile.protocol_dialect(),
                proxy_id: proxy.id(),
                proxy_config_version: proxy.config_version(),
                enabled: account.enabled(),
                expires_at: account.expires_at(),
                endpoint_enabled: true,
                proxy_enabled: proxy.enabled(),
                models: account.models().to_vec(),
                available_models,
            },
            requests_per_minute: account.requests_per_minute(),
            generation: CredentialGenerationDefinition::new(
                account.account_generation(),
                account.token_version(),
                CredentialAuthentication::oauth(Arc::new(token)),
            ),
        });
    }
    materials.ensure_consumed()
}

struct OAuthMaterials {
    by_id: HashMap<OAuthAccountId, any2api_storage::api::StoredOAuthAccountMaterial>,
}

impl OAuthMaterials {
    fn new(stored: StoredOAuthAccountMaterials) -> Result<Self, RoutingCredentialCompileError> {
        let mut by_id = HashMap::new();
        for material in stored.into_entries() {
            let id = material.account_id();
            if by_id.insert(id, material).is_some() {
                return Err(RoutingCredentialCompileError::DuplicateOAuthMaterial(id));
            }
        }
        Ok(Self { by_id })
    }

    fn take_for(
        &mut self,
        account: &OAuthAccount,
    ) -> Result<SecretBytes, RoutingCredentialCompileError> {
        let material = self.by_id.remove(&account.id()).ok_or(
            RoutingCredentialCompileError::MissingOAuthMaterial(account.id()),
        )?;
        validate_oauth_material(account, &material)?;
        Ok(material.into_document().into_bytes())
    }

    fn ensure_consumed(self) -> Result<(), RoutingCredentialCompileError> {
        self.by_id.keys().next().copied().map_or(Ok(()), |id| {
            Err(RoutingCredentialCompileError::UnknownOAuthMaterial(id))
        })
    }
}

fn validate_oauth_material(
    account: &OAuthAccount,
    material: &StoredOAuthAccountMaterial,
) -> Result<(), RoutingCredentialCompileError> {
    if material.provider_kind() != account.provider_kind() {
        return Err(RoutingCredentialCompileError::OAuthProviderMismatch(
            account.id(),
        ));
    }
    if material.token_version() != account.token_version() {
        return Err(RoutingCredentialCompileError::OAuthVersionMismatch(
            account.id(),
        ));
    }
    if material.account_generation() != account.account_generation() {
        return Err(RoutingCredentialCompileError::OAuthGenerationMismatch(
            account.id(),
        ));
    }
    Ok(())
}

const OAUTH_ENDPOINT_NAMESPACE: Uuid = Uuid::from_u128(0xc682_f3d3_bda4_54be_bfd4_57c6_e06a_1f3f);

fn oauth_endpoint_id(provider: ProviderKind) -> ProviderEndpointId {
    ProviderEndpointId::from_uuid(Uuid::new_v5(
        &OAUTH_ENDPOINT_NAMESPACE,
        provider.as_str().as_bytes(),
    ))
}
