use crate::{PublicModelName, UpstreamModelName};

use super::entity::ProviderCredentialValidationError;

/// One confirmed model on a provider credential: the upstream name the
/// provider expects plus an optional public alias clients use instead.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ProviderCredentialModel {
    upstream_model: UpstreamModelName,
    public_model: Option<PublicModelName>,
}

impl ProviderCredentialModel {
    pub fn new(
        upstream_model: impl Into<String>,
        public_model: Option<String>,
    ) -> Result<Self, ProviderCredentialValidationError> {
        let upstream_model = UpstreamModelName::new(upstream_model)
            .map_err(ProviderCredentialValidationError::InvalidModel)?;
        let public_model = public_model
            .map(PublicModelName::new)
            .transpose()
            .map_err(ProviderCredentialValidationError::InvalidPublicModel)?
            .filter(|alias| alias.as_str() != upstream_model.as_str());
        Ok(Self {
            upstream_model,
            public_model,
        })
    }

    #[must_use]
    pub const fn upstream_model(&self) -> &UpstreamModelName {
        &self.upstream_model
    }

    /// The alias exactly as persisted; `None` means the model is published
    /// under its upstream name.
    #[must_use]
    pub const fn public_alias(&self) -> Option<&PublicModelName> {
        self.public_model.as_ref()
    }

    /// The name clients see: the alias when present, the upstream name otherwise.
    #[must_use]
    pub fn public_model(&self) -> PublicModelName {
        self.public_model
            .clone()
            .unwrap_or_else(|| PublicModelName::from(&self.upstream_model))
    }
}

#[cfg(test)]
mod tests {
    use super::ProviderCredentialModel;
    use crate::provider::credential::ProviderCredentialValidationError;

    #[test]
    fn identity_aliases_are_normalized_away_and_invalid_aliases_are_rejected() {
        let renamed =
            ProviderCredentialModel::new("gpt-5.6-sol-ganen", Some("gpt-5.6-sol".to_owned()))
                .expect("aliased model");
        assert_eq!(renamed.upstream_model().as_str(), "gpt-5.6-sol-ganen");
        assert_eq!(renamed.public_model().as_str(), "gpt-5.6-sol");
        assert!(renamed.public_alias().is_some());

        let identity = ProviderCredentialModel::new("gpt-5.6-sol", Some("gpt-5.6-sol".to_owned()))
            .expect("identity alias");
        assert_eq!(identity.public_alias(), None);
        assert_eq!(identity.public_model().as_str(), "gpt-5.6-sol");

        assert!(matches!(
            ProviderCredentialModel::new("gpt-5.6-sol", Some(" padded ".to_owned())),
            Err(ProviderCredentialValidationError::InvalidPublicModel(_))
        ));
    }
}
