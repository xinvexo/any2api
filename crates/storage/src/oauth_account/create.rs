use any2api_domain::{OAuthAccountDraft, OAuthAccountId, OAuthProxySelection, ProviderKind};

use super::document::OAuthAccountDocument;

pub struct OAuthAccountCreate {
    pub(crate) id: OAuthAccountId,
    pub(crate) provider_kind: ProviderKind,
    pub(crate) draft: OAuthAccountDraft,
    pub(crate) proxy_selection: OAuthProxySelection,
    pub(crate) safe_account_email: Option<String>,
    pub(crate) expires_at: Option<i64>,
    pub(crate) models: Vec<String>,
    pub(crate) document: OAuthAccountDocument,
}

impl OAuthAccountCreate {
    #[allow(clippy::too_many_arguments)]
    #[must_use]
    pub fn new(
        id: OAuthAccountId,
        provider_kind: ProviderKind,
        draft: OAuthAccountDraft,
        proxy_selection: OAuthProxySelection,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        models: Vec<String>,
        document: OAuthAccountDocument,
    ) -> Self {
        Self {
            id,
            provider_kind,
            draft,
            proxy_selection,
            safe_account_email,
            expires_at,
            models,
            document,
        }
    }
}
