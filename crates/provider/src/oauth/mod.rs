mod device;
mod identity;
pub(crate) mod import;
#[cfg(test)]
mod import_tests;
mod model_catalog;
pub(crate) mod quota;
mod refresh;
mod routing;
mod token;
#[cfg(test)]
mod token_tests;

pub use device::{OAuthDeviceAuthorization, OAuthDeviceTokenPoll, OAuthLoginFlow};
pub use identity::OAuthPrincipalIdentity;
pub(crate) use identity::{
    default_principal_identity, email_principal_identity, workspace_member_principal_identity,
};
pub use import::{
    MAX_OAUTH_IMPORT_ACCOUNTS_PER_DOCUMENT, OAuthImportParseError, OAuthImportedAccount,
    parse_oauth_import_document,
};
pub use model_catalog::OAuthModelCatalogScope;
pub use quota::{
    OAuthQuotaAccessStatus, OAuthQuotaAccountStatus, OAuthQuotaAuthenticationStatus,
    OAuthQuotaBilling, OAuthQuotaCredits, OAuthQuotaExhaustion, OAuthQuotaQueryPlan,
    OAuthQuotaRateLimit, OAuthQuotaReachedType, OAuthQuotaRejection, OAuthQuotaResetCredit,
    OAuthQuotaResetCredits, OAuthQuotaResetResult, OAuthQuotaSupplement, OAuthQuotaTokenBalance,
    OAuthQuotaTokenBalanceSource, OAuthQuotaUsage, OAuthQuotaWindow, OAuthQuotaWindowKind,
};
pub use refresh::OAuthRefreshRejection;
pub use routing::OAuthRoutingProfile;
pub use token::{
    OAuthGrant, OAuthRequestPlan, OAuthTokenMaterial, decode_oauth_account_document,
    encode_oauth_account_document,
};
pub(crate) use token::{expires_at_from_duration, form_headers, json_headers};
