use crate::{GatewayApiKeyId, PublicModelName};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestLogOutcomeFilter {
    Success,
    Failed,
    Cancelled,
}

impl RequestLogOutcomeFilter {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Success => "success",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "success" => Some(Self::Success),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RequestLogFilter {
    outcome: Option<RequestLogOutcomeFilter>,
    public_model: Option<PublicModelName>,
    gateway_api_key_id: Option<GatewayApiKeyId>,
}

impl RequestLogFilter {
    #[must_use]
    pub fn new(
        outcome: Option<RequestLogOutcomeFilter>,
        public_model: Option<PublicModelName>,
        gateway_api_key_id: Option<GatewayApiKeyId>,
    ) -> Self {
        Self {
            outcome,
            public_model,
            gateway_api_key_id,
        }
    }

    #[must_use]
    pub const fn outcome(&self) -> Option<RequestLogOutcomeFilter> {
        self.outcome
    }

    #[must_use]
    pub fn public_model(&self) -> Option<&PublicModelName> {
        self.public_model.as_ref()
    }

    #[must_use]
    pub const fn gateway_api_key_id(&self) -> Option<GatewayApiKeyId> {
        self.gateway_api_key_id
    }
}
