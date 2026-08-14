use crate::{CredentialId, GatewayApiKeyId, OAuthAccountId, ProtocolOperation, PublicModelName};

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
    operation: Option<ProtocolOperation>,
    public_model: Option<PublicModelName>,
    gateway_api_key_id: Option<GatewayApiKeyId>,
    credential_id: Option<CredentialId>,
    oauth_account_id: Option<OAuthAccountId>,
}

impl RequestLogFilter {
    #[must_use]
    pub fn new(
        outcome: Option<RequestLogOutcomeFilter>,
        operation: Option<ProtocolOperation>,
        public_model: Option<PublicModelName>,
        gateway_api_key_id: Option<GatewayApiKeyId>,
        credential_id: Option<CredentialId>,
        oauth_account_id: Option<OAuthAccountId>,
    ) -> Option<Self> {
        if credential_id.is_some() && oauth_account_id.is_some() {
            return None;
        }
        Some(Self {
            outcome,
            operation,
            public_model,
            gateway_api_key_id,
            credential_id,
            oauth_account_id,
        })
    }

    #[must_use]
    pub const fn outcome(&self) -> Option<RequestLogOutcomeFilter> {
        self.outcome
    }

    #[must_use]
    pub const fn operation(&self) -> Option<ProtocolOperation> {
        self.operation
    }

    #[must_use]
    pub fn public_model(&self) -> Option<&PublicModelName> {
        self.public_model.as_ref()
    }

    #[must_use]
    pub const fn gateway_api_key_id(&self) -> Option<GatewayApiKeyId> {
        self.gateway_api_key_id
    }

    #[must_use]
    pub const fn credential_id(&self) -> Option<CredentialId> {
        self.credential_id
    }

    #[must_use]
    pub const fn oauth_account_id(&self) -> Option<OAuthAccountId> {
        self.oauth_account_id
    }
}

#[cfg(test)]
mod tests {
    use crate::{CredentialId, OAuthAccountId};

    use super::RequestLogFilter;

    #[test]
    fn upstream_credential_filters_are_mutually_exclusive() {
        assert!(
            RequestLogFilter::new(
                None,
                None,
                None,
                None,
                Some(CredentialId::new()),
                Some(OAuthAccountId::new()),
            )
            .is_none()
        );
    }
}
