mod control_plane;
mod coordinator;
mod document;
mod error;
mod import;
mod login;
mod quota;
pub(crate) mod refresh;

pub use coordinator::OAuthService;
pub use error::OAuthError;
pub use import::{
    MAX_OAUTH_IMPORT_ACCOUNTS, OAuthImportError, OAuthImportFailureKind, OAuthImportResult,
};
pub use login::{OAuthActivationResult, OAuthDevicePollResult, OAuthStartFlow, OAuthStartResult};
pub(crate) use quota::{OAuthQuotaActivity, OAuthQuotaActivityGuard};
pub use quota::{OAuthQuotaError, OAuthQuotaResetOutcome, OAuthQuotaSnapshot};
pub use refresh::{
    OAuthRefreshFailure, OAuthRefreshFailureReason, OAuthRefreshFailureScope,
    OAuthRefreshFailureStage, OAuthRefreshTrigger,
};
