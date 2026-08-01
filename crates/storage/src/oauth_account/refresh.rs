use any2api_domain::OAuthAccountId;

use super::document::OAuthAccountDocument;

pub struct OAuthAccountRefresh {
    id: OAuthAccountId,
    expected_token_version: u64,
    safe_account_email: Option<String>,
    expires_at: Option<i64>,
    document: OAuthAccountDocument,
}

impl OAuthAccountRefresh {
    #[must_use]
    pub fn new(
        id: OAuthAccountId,
        expected_token_version: u64,
        safe_account_email: Option<String>,
        expires_at: Option<i64>,
        document: OAuthAccountDocument,
    ) -> Self {
        Self {
            id,
            expected_token_version,
            safe_account_email,
            expires_at,
            document,
        }
    }

    pub(super) fn into_parts(
        self,
    ) -> (
        OAuthAccountId,
        u64,
        Option<String>,
        Option<i64>,
        OAuthAccountDocument,
    ) {
        (
            self.id,
            self.expected_token_version,
            self.safe_account_email,
            self.expires_at,
            self.document,
        )
    }
}
