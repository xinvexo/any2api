mod control_plane;
mod coordinator;
mod document;
mod error;
mod import;
mod login;
mod quota;
pub(crate) mod refresh;

pub use coordinator::{OAuthQuotaManualRefreshResult, OAuthQuotaRefreshBatchResult, OAuthService};
pub use error::OAuthError;
pub use import::{
    MAX_OAUTH_IMPORT_ACCOUNTS, OAuthImportError, OAuthImportFailureKind, OAuthImportResult,
};
pub use login::{OAuthActivationResult, OAuthDevicePollResult, OAuthStartFlow, OAuthStartResult};
pub use quota::{
    OAuthModelCatalogSnapshot, OAuthQuotaError, OAuthQuotaEstimate, OAuthQuotaRateCard,
    OAuthQuotaResetOutcome, OAuthQuotaSnapshot,
};
pub(crate) use quota::{OAuthQuotaActivity, OAuthQuotaActivityGuard};
pub use refresh::{
    OAuthRefreshFailure, OAuthRefreshFailureReason, OAuthRefreshFailureScope,
    OAuthRefreshFailureStage, OAuthRefreshTrigger,
};
