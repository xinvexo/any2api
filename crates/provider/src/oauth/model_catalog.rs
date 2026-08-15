use any2api_domain::ProviderKind;

use crate::ProviderError;

const MAX_DIRECTORY_SCOPE_CHARS: usize = 96;

/// Non-secret Provider-defined identity for a shared OAuth model directory.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct OAuthModelCatalogScope {
    provider: ProviderKind,
    directory_scope: String,
}

impl OAuthModelCatalogScope {
    pub fn new(
        provider: ProviderKind,
        directory_scope: impl Into<String>,
    ) -> Result<Self, ProviderError> {
        let directory_scope = directory_scope.into();
        if directory_scope.is_empty()
            || directory_scope.len() > MAX_DIRECTORY_SCOPE_CHARS
            || !directory_scope
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_')
        {
            return Err(ProviderError::InvalidResponse(
                "OAuth model directory scope is invalid".into(),
            ));
        }
        Ok(Self {
            provider,
            directory_scope,
        })
    }

    #[must_use]
    pub const fn provider(&self) -> ProviderKind {
        self.provider
    }

    #[must_use]
    pub fn directory_scope(&self) -> &str {
        &self.directory_scope
    }
}
