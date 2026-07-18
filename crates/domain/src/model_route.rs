use std::collections::{HashMap, HashSet};

use thiserror::Error;

use crate::{
    ModelNameValidationError, ModelRouteId, ProtocolDialect, PublicModelName, RouteTarget,
    RouteTargetDraft,
};

const MAX_MODEL_ROUTE_CONFIG_VERSION: u64 = u32::MAX as u64;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRouteDraft {
    public_model: PublicModelName,
    ingress_protocol: ProtocolDialect,
    fallback_on_saturation: Option<bool>,
    enabled: bool,
    targets: Vec<RouteTargetDraft>,
}

impl ModelRouteDraft {
    pub fn new(
        public_model: impl Into<String>,
        ingress_protocol: ProtocolDialect,
        fallback_on_saturation: Option<bool>,
        enabled: bool,
        mut targets: Vec<RouteTargetDraft>,
    ) -> Result<Self, ModelRouteValidationError> {
        if !matches!(
            ingress_protocol,
            ProtocolDialect::OpenAiResponses | ProtocolDialect::AnthropicMessages
        ) {
            return Err(ModelRouteValidationError::UnsupportedIngressProtocol);
        }
        validate_targets(enabled, &targets)?;
        sort_target_drafts(&mut targets);
        Ok(Self {
            public_model: PublicModelName::new(public_model)
                .map_err(ModelRouteValidationError::InvalidPublicModel)?,
            ingress_protocol,
            fallback_on_saturation,
            enabled,
            targets,
        })
    }

    #[must_use]
    pub const fn public_model(&self) -> &PublicModelName {
        &self.public_model
    }

    #[must_use]
    pub const fn ingress_protocol(&self) -> ProtocolDialect {
        self.ingress_protocol
    }

    #[must_use]
    pub const fn fallback_on_saturation(&self) -> Option<bool> {
        self.fallback_on_saturation
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub fn targets(&self) -> &[RouteTargetDraft] {
        &self.targets
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ModelRoute {
    id: ModelRouteId,
    public_model: PublicModelName,
    ingress_protocol: ProtocolDialect,
    fallback_on_saturation: Option<bool>,
    enabled: bool,
    config_version: u64,
    targets: Vec<RouteTarget>,
}

impl ModelRoute {
    pub fn create(id: ModelRouteId, draft: ModelRouteDraft) -> Self {
        Self::from_draft(id, draft, 1)
    }

    pub fn restore(
        id: ModelRouteId,
        draft: ModelRouteDraft,
        config_version: u64,
    ) -> Result<Self, ModelRouteValidationError> {
        if !valid_version(config_version) {
            return Err(ModelRouteValidationError::InvalidConfigVersion);
        }
        Ok(Self::from_draft(id, draft, config_version))
    }

    pub fn updated(&self, draft: ModelRouteDraft) -> Result<Self, ModelRouteValidationError> {
        if self.ingress_protocol != draft.ingress_protocol {
            return Err(ModelRouteValidationError::IngressProtocolChanged);
        }
        let existing = self
            .targets
            .iter()
            .map(|target| (target.id(), target))
            .collect::<HashMap<_, _>>();
        let targets = draft
            .targets
            .iter()
            .cloned()
            .map(|target| match existing.get(&target.id()) {
                Some(current) => current.updated(target),
                None => Ok(RouteTarget::from_draft(self.id, target)),
            })
            .collect::<Result<Vec<_>, _>>()?;
        let mut updated = Self {
            id: self.id,
            public_model: draft.public_model,
            ingress_protocol: draft.ingress_protocol,
            fallback_on_saturation: draft.fallback_on_saturation,
            enabled: draft.enabled,
            config_version: self.config_version,
            targets,
        };
        updated.sort_targets();
        if &updated == self {
            return Ok(self.clone());
        }
        updated.config_version = next_version(self.config_version)?;
        Ok(updated)
    }

    #[must_use]
    pub const fn id(&self) -> ModelRouteId {
        self.id
    }

    #[must_use]
    pub const fn public_model(&self) -> &PublicModelName {
        &self.public_model
    }

    #[must_use]
    pub const fn ingress_protocol(&self) -> ProtocolDialect {
        self.ingress_protocol
    }

    #[must_use]
    pub const fn fallback_on_saturation(&self) -> Option<bool> {
        self.fallback_on_saturation
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }

    #[must_use]
    pub const fn config_version(&self) -> u64 {
        self.config_version
    }

    #[must_use]
    pub fn targets(&self) -> &[RouteTarget] {
        &self.targets
    }

    fn from_draft(id: ModelRouteId, draft: ModelRouteDraft, config_version: u64) -> Self {
        let targets = draft
            .targets
            .into_iter()
            .map(|target| RouteTarget::from_draft(id, target))
            .collect();
        Self {
            id,
            public_model: draft.public_model,
            ingress_protocol: draft.ingress_protocol,
            fallback_on_saturation: draft.fallback_on_saturation,
            enabled: draft.enabled,
            config_version,
            targets,
        }
    }

    fn sort_targets(&mut self) {
        self.targets.sort_by(|left, right| {
            left.fallback_tier()
                .cmp(&right.fallback_tier())
                .then_with(|| {
                    left.provider_endpoint_id()
                        .cmp(&right.provider_endpoint_id())
                })
                .then_with(|| left.upstream_model().cmp(right.upstream_model()))
        });
    }
}

#[derive(Clone, Debug, Eq, Error, PartialEq)]
pub enum ModelRouteValidationError {
    #[error("public model name is invalid: {0}")]
    InvalidPublicModel(ModelNameValidationError),
    #[error("upstream model name is invalid: {0}")]
    InvalidUpstreamModel(ModelNameValidationError),
    #[error("ingress protocol is not supported in the first release")]
    UnsupportedIngressProtocol,
    #[error("model route must contain at least one target")]
    EmptyTargets,
    #[error("enabled model route must contain an enabled target")]
    NoEnabledTarget,
    #[error("route target id is duplicated")]
    DuplicateTargetId,
    #[error("route target is duplicated")]
    DuplicateTarget,
    #[error("model route configuration version is invalid")]
    InvalidConfigVersion,
    #[error("model route ingress protocol cannot change")]
    IngressProtocolChanged,
    #[error("route target endpoint or upstream model cannot change under the same id")]
    TargetIdentityChanged,
    #[error("model route id is duplicated")]
    DuplicateRouteId,
    #[error("public model is duplicated for this ingress protocol")]
    DuplicatePublicModel,
    #[error("route target id is reused by another route")]
    ReusedTargetId,
    #[error("route target references a missing provider endpoint")]
    MissingProviderEndpoint(crate::ProviderEndpointId),
    #[error("route target protocol is incompatible with its provider endpoint")]
    IncompatibleTargetProtocol(crate::ProviderEndpointId),
}

fn validate_targets(
    route_enabled: bool,
    targets: &[RouteTargetDraft],
) -> Result<(), ModelRouteValidationError> {
    if targets.is_empty() {
        return Err(ModelRouteValidationError::EmptyTargets);
    }
    if route_enabled && !targets.iter().any(RouteTargetDraft::enabled) {
        return Err(ModelRouteValidationError::NoEnabledTarget);
    }
    let mut ids = HashSet::new();
    let mut identities = HashSet::new();
    for target in targets {
        if !ids.insert(target.id()) {
            return Err(ModelRouteValidationError::DuplicateTargetId);
        }
        if !identities.insert((
            target.provider_endpoint_id(),
            target.upstream_model().clone(),
        )) {
            return Err(ModelRouteValidationError::DuplicateTarget);
        }
    }
    Ok(())
}

fn sort_target_drafts(targets: &mut [RouteTargetDraft]) {
    targets.sort_by(|left, right| {
        left.fallback_tier()
            .cmp(&right.fallback_tier())
            .then_with(|| {
                left.provider_endpoint_id()
                    .cmp(&right.provider_endpoint_id())
            })
            .then_with(|| left.upstream_model().cmp(right.upstream_model()))
    });
}

const fn valid_version(value: u64) -> bool {
    value > 0 && value <= MAX_MODEL_ROUTE_CONFIG_VERSION
}

fn next_version(value: u64) -> Result<u64, ModelRouteValidationError> {
    value
        .checked_add(1)
        .filter(|next| valid_version(*next))
        .ok_or(ModelRouteValidationError::InvalidConfigVersion)
}

#[cfg(test)]
mod tests {
    use super::{ModelRoute, ModelRouteDraft, ModelRouteValidationError};
    use crate::{
        FallbackTier, ModelRouteId, ProtocolDialect, ProviderEndpointId, RouteTargetDraft,
        RouteTargetId,
    };

    #[test]
    fn target_identity_changes_require_a_new_target_id() {
        let target_id = RouteTargetId::new();
        let route = ModelRoute::create(
            ModelRouteId::new(),
            draft(target_id, ProviderEndpointId::new(), "gpt-5.1", 0),
        );
        let replacement = draft(target_id, ProviderEndpointId::new(), "gpt-5.1", 1);

        assert_eq!(
            route.updated(replacement).expect_err("identity change"),
            ModelRouteValidationError::TargetIdentityChanged
        );
    }

    #[test]
    fn policy_changes_preserve_target_identity_and_bump_the_aggregate_version() {
        let target_id = RouteTargetId::new();
        let endpoint_id = ProviderEndpointId::new();
        let route = ModelRoute::create(
            ModelRouteId::new(),
            draft(target_id, endpoint_id, "gpt-5.1", 0),
        );
        let updated = route
            .updated(draft(target_id, endpoint_id, "gpt-5.1", 2))
            .expect("policy update");

        assert_eq!(updated.config_version(), 2);
        assert_eq!(updated.targets()[0].id(), target_id);
        assert_eq!(updated.targets()[0].fallback_tier().get(), 2);
    }

    fn draft(
        target_id: RouteTargetId,
        endpoint_id: ProviderEndpointId,
        upstream_model: &str,
        tier: u16,
    ) -> ModelRouteDraft {
        ModelRouteDraft::new(
            "codex-public",
            ProtocolDialect::OpenAiResponses,
            None,
            true,
            vec![
                RouteTargetDraft::new(
                    target_id,
                    endpoint_id,
                    upstream_model,
                    FallbackTier::new(tier),
                    true,
                )
                .expect("target draft"),
            ],
        )
        .expect("route draft")
    }
}
