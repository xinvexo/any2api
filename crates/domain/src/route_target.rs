use crate::{
    ModelRouteId, ModelRouteValidationError, ProviderEndpointId, RouteTargetId, UpstreamModelName,
};

#[derive(Clone, Copy, Debug, Default, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct FallbackTier(u16);

impl FallbackTier {
    #[must_use]
    pub const fn new(value: u16) -> Self {
        Self(value)
    }

    #[must_use]
    pub const fn get(self) -> u16 {
        self.0
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteTargetDraft {
    id: RouteTargetId,
    provider_endpoint_id: ProviderEndpointId,
    upstream_model: UpstreamModelName,
    fallback_tier: FallbackTier,
    enabled: bool,
}

impl RouteTargetDraft {
    pub fn new(
        id: RouteTargetId,
        provider_endpoint_id: ProviderEndpointId,
        upstream_model: impl Into<String>,
        fallback_tier: FallbackTier,
        enabled: bool,
    ) -> Result<Self, ModelRouteValidationError> {
        Ok(Self {
            id,
            provider_endpoint_id,
            upstream_model: UpstreamModelName::new(upstream_model)
                .map_err(ModelRouteValidationError::InvalidUpstreamModel)?,
            fallback_tier,
            enabled,
        })
    }

    #[must_use]
    pub const fn id(&self) -> RouteTargetId {
        self.id
    }

    #[must_use]
    pub const fn provider_endpoint_id(&self) -> ProviderEndpointId {
        self.provider_endpoint_id
    }

    #[must_use]
    pub const fn upstream_model(&self) -> &UpstreamModelName {
        &self.upstream_model
    }

    #[must_use]
    pub const fn fallback_tier(&self) -> FallbackTier {
        self.fallback_tier
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RouteTarget {
    id: RouteTargetId,
    model_route_id: ModelRouteId,
    provider_endpoint_id: ProviderEndpointId,
    upstream_model: UpstreamModelName,
    fallback_tier: FallbackTier,
    enabled: bool,
}

impl RouteTarget {
    pub(crate) fn from_draft(model_route_id: ModelRouteId, draft: RouteTargetDraft) -> Self {
        Self {
            id: draft.id,
            model_route_id,
            provider_endpoint_id: draft.provider_endpoint_id,
            upstream_model: draft.upstream_model,
            fallback_tier: draft.fallback_tier,
            enabled: draft.enabled,
        }
    }

    pub(crate) fn updated(
        &self,
        draft: RouteTargetDraft,
    ) -> Result<Self, ModelRouteValidationError> {
        if self.provider_endpoint_id != draft.provider_endpoint_id
            || self.upstream_model != draft.upstream_model
        {
            return Err(ModelRouteValidationError::TargetIdentityChanged);
        }
        Ok(Self::from_draft(self.model_route_id, draft))
    }

    #[must_use]
    pub const fn id(&self) -> RouteTargetId {
        self.id
    }

    #[must_use]
    pub const fn model_route_id(&self) -> ModelRouteId {
        self.model_route_id
    }

    #[must_use]
    pub const fn provider_endpoint_id(&self) -> ProviderEndpointId {
        self.provider_endpoint_id
    }

    #[must_use]
    pub const fn upstream_model(&self) -> &UpstreamModelName {
        &self.upstream_model
    }

    #[must_use]
    pub const fn fallback_tier(&self) -> FallbackTier {
        self.fallback_tier
    }

    #[must_use]
    pub const fn enabled(&self) -> bool {
        self.enabled
    }
}
