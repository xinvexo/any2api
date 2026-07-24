use std::{collections::HashMap, sync::Arc};

use any2api_domain::{
    OAuthAccount, OAuthAccountConfiguration, OAuthAccountId, ProviderCredentialConfiguration,
    ProviderEndpointConfiguration, ProviderEndpointId, ProviderKind, ProxyConfiguration,
    ProxyProfileId,
};
use any2api_provider::api::ProviderRegistry;
use any2api_storage::api::StoredOAuthAccountMaterials;
use secrecy::ExposeSecret;
use uuid::Uuid;

use super::RoutingCredentialSpec;
use crate::{
    credential_auth::CredentialAuthMaterials,
    credential_runtime::{CredentialAuthentication, CredentialGenerationDefinition},
};

impl RoutingCredentialSpec {
    pub(crate) fn compile(
        credentials: &ProviderCredentialConfiguration,
        endpoints: &ProviderEndpointConfiguration,
        oauth_accounts: &OAuthAccountConfiguration,
        proxies: &ProxyConfiguration,
        mut credential_auth: CredentialAuthMaterials,
        oauth_materials: StoredOAuthAccountMaterials,
        providers: &ProviderRegistry,
    ) -> Vec<Self> {
        let mut specs =
            Vec::with_capacity(credentials.credentials().len() + oauth_accounts.accounts().len());
        compile_provider_credentials(
            &mut specs,
            credentials,
            endpoints,
            proxies,
            &mut credential_auth,
        );
        credential_auth.assert_consumed();
        compile_oauth_accounts(
            &mut specs,
            oauth_accounts,
            proxies,
            oauth_materials,
            providers,
        );
        specs
    }
}

fn compile_provider_credentials(
    specs: &mut Vec<RoutingCredentialSpec>,
    credentials: &ProviderCredentialConfiguration,
    endpoints: &ProviderEndpointConfiguration,
    proxies: &ProxyConfiguration,
    credential_auth: &mut CredentialAuthMaterials,
) {
    for credential in credentials.credentials() {
        let endpoint = endpoints
            .get(credential.provider_endpoint_id())
            .expect("configuration validates ProviderCredential endpoint");
        let proxy = proxies
            .resolve(credential.proxy_profile_id())
            .expect("configuration validates ProviderCredential proxy");
        let auth = credential_auth.take_for(credential);
        specs.push(RoutingCredentialSpec {
            id: credential.id().into(),
            provider_kind: endpoint.provider_kind(),
            label: credential.label().to_owned(),
            endpoint_id: endpoint.id(),
            endpoint_name: endpoint.name().to_owned(),
            endpoint_config_version: endpoint.config_version(),
            base_url: endpoint.base_url().clone(),
            ingress_protocol: endpoint.protocol_dialect(),
            upstream_protocol: endpoint.effective_upstream_protocol_dialect(),
            proxy_id: proxy.id(),
            enabled: credential.enabled(),
            expires_at: None,
            endpoint_enabled: endpoint.enabled(),
            models: credential.models().to_vec(),
            available_models: credential.models().to_vec(),
            max_concurrency: credential.max_concurrency(),
            generation: Some(CredentialGenerationDefinition::new(
                credential.credential_generation(),
                credential.secret_version(),
                CredentialAuthentication::provider_api_key(auth.into_provider_secret()),
            )),
        });
    }
}

fn compile_oauth_accounts(
    specs: &mut Vec<RoutingCredentialSpec>,
    accounts: &OAuthAccountConfiguration,
    proxies: &ProxyConfiguration,
    materials: StoredOAuthAccountMaterials,
    providers: &ProviderRegistry,
) {
    let mut materials = OAuthMaterials::new(materials);
    for account in accounts.accounts() {
        let Some(driver) = providers.get(account.provider_kind()) else {
            continue;
        };
        let material = materials.take_for(account);
        let token = driver
            .parse_oauth_token(material.expose_secret())
            .expect("storage validated Provider OAuth account document")
            .with_expires_at_fallback(account.expires_at());
        let profile = driver
            .oauth_routing_profile(&token)
            .expect("registered Provider has a valid fixed OAuth routing profile");
        let proxy = proxies
            .resolve(ProxyProfileId::DIRECT)
            .expect("configuration always resolves DIRECT/global proxy");
        let available_models = profile.models().to_vec();
        let models = account
            .models()
            .iter()
            .filter(|model| profile.models().binary_search(model).is_ok())
            .cloned()
            .collect();
        specs.push(RoutingCredentialSpec {
            id: account.id().into(),
            provider_kind: account.provider_kind(),
            label: account.label().to_owned(),
            endpoint_id: oauth_endpoint_id(account.provider_kind()),
            endpoint_name: format!("{:?} OAuth", account.provider_kind()),
            endpoint_config_version: 1,
            base_url: profile.base_url().clone(),
            ingress_protocol: profile.protocol_dialect(),
            upstream_protocol: profile.protocol_dialect(),
            proxy_id: proxy.id(),
            enabled: account.enabled(),
            expires_at: account.expires_at(),
            endpoint_enabled: true,
            models,
            available_models,
            max_concurrency: account.max_concurrency(),
            generation: Some(CredentialGenerationDefinition::new(
                account.account_generation(),
                account.token_version(),
                CredentialAuthentication::oauth(Arc::new(token)),
            )),
        });
    }
    materials.assert_consumed();
}

struct OAuthMaterials {
    by_id: HashMap<OAuthAccountId, any2api_storage::api::StoredOAuthAccountMaterial>,
}

impl OAuthMaterials {
    fn new(stored: StoredOAuthAccountMaterials) -> Self {
        let by_id = stored
            .into_entries()
            .into_iter()
            .map(|entry| (entry.account_id(), entry))
            .collect();
        Self { by_id }
    }

    fn take_for(&mut self, account: &OAuthAccount) -> any2api_storage::api::SecretBytes {
        let material = self
            .by_id
            .remove(&account.id())
            .expect("storage omitted OAuth account material");
        assert_eq!(
            material.provider_kind(),
            account.provider_kind(),
            "OAuth material Provider mismatch"
        );
        assert_eq!(
            material.token_version(),
            account.token_version(),
            "OAuth material token version mismatch"
        );
        assert_eq!(
            material.account_generation(),
            account.account_generation(),
            "OAuth material generation mismatch"
        );
        material.into_document().into_bytes()
    }

    fn assert_consumed(self) {
        assert!(
            self.by_id.is_empty(),
            "storage returned material for an unknown OAuth account"
        );
    }
}

const OAUTH_ENDPOINT_NAMESPACE: Uuid = Uuid::from_u128(0xc682_f3d3_bda4_54be_bfd4_57c6_e06a_1f3f);

fn oauth_endpoint_id(provider: ProviderKind) -> ProviderEndpointId {
    ProviderEndpointId::from_uuid(Uuid::new_v5(
        &OAUTH_ENDPOINT_NAMESPACE,
        provider.as_str().as_bytes(),
    ))
}
