use std::{collections::BTreeSet, sync::Arc};

use any2api_domain::{
    FallbackTier, ModelRouteConfiguration, ModelRouteId, OAuthAccountId, PublicModelName,
    RoutingCredentialId, UpstreamModelName,
};
use any2api_provider::api::OAuthTokenMaterial;

use super::PublishedSnapshot;
use crate::{
    route_candidates::oauth_route_id,
    routing_credential::{RoutingCredential, RoutingCredentials},
};

impl PublishedSnapshot {
    #[must_use]
    pub fn oauth_available_models(&self, id: OAuthAccountId) -> Option<&[UpstreamModelName]> {
        self.routing_credentials
            .get(RoutingCredentialId::oauth_account(id))
            .map(RoutingCredential::available_models)
    }

    #[must_use]
    pub fn oauth_token_material(&self, id: OAuthAccountId) -> Option<Arc<OAuthTokenMaterial>> {
        let account = self.oauth_accounts.get(id)?;
        let credential = self
            .routing_credentials
            .get(RoutingCredentialId::oauth_account(id))?;
        let generation = credential.binding().generation();
        if generation.authentication_version() != account.token_version() {
            return None;
        }
        generation.oauth_token()
    }

    /// Official Codex `chatgpt_plan_type` from the ID Token. Claude has none.
    #[must_use]
    pub fn oauth_plan_label(&self, id: OAuthAccountId) -> Option<String> {
        let account = self.oauth_accounts.get(id)?;
        if account.provider_kind() != any2api_domain::ProviderKind::Codex {
            return None;
        }
        let token = self.oauth_token_material(id)?;
        any2api_provider::api::codex_oauth_plan_label(token.as_ref())
    }

    #[must_use]
    pub fn public_model_names(&self) -> BTreeSet<String> {
        let mut names = self
            .model_routes
            .routes()
            .iter()
            .filter(|route| route.enabled())
            .map(|route| route.public_model().as_str().to_owned())
            .collect::<BTreeSet<_>>();
        for credential in self
            .routing_credentials()
            .iter()
            .filter(|credential| credential.is_oauth() && credential.routable())
        {
            names.extend(
                credential
                    .models()
                    .iter()
                    .map(|model| model.as_str().to_owned()),
            );
        }
        names
    }
}

pub(super) fn route_tiers(
    model_routes: &ModelRouteConfiguration,
    credentials: &RoutingCredentials,
) -> Vec<(ModelRouteId, FallbackTier)> {
    credentials
        .as_slice()
        .iter()
        .filter(|credential| credential.is_oauth())
        .flat_map(|credential| {
            credential.models().iter().filter_map(|model| {
                let public_model = PublicModelName::new(model.as_str().to_owned()).ok()?;
                let route_id = model_routes
                    .resolve(credential.ingress_protocol(), &public_model)
                    .map_or_else(
                        || oauth_route_id(credential.ingress_protocol(), &public_model),
                        any2api_domain::ModelRoute::id,
                    );
                Some((route_id, FallbackTier::default()))
            })
        })
        .collect()
}
