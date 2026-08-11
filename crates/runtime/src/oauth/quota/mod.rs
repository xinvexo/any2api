mod activity;
mod authentication;
mod coordinator;
mod estimator;
mod health;
mod observation;
mod operation_gate;
mod persistence;
mod rejection;
mod request;
mod types;

#[cfg(test)]
mod authentication_tests;
#[cfg(test)]
mod claude_tests;
#[cfg(test)]
mod grok_tests;
#[cfg(test)]
mod mock_transport;
#[cfg(test)]
mod test_support;
#[cfg(test)]
mod tests;

pub(crate) use activity::{OAuthQuotaActivity, OAuthQuotaActivityGuard};
pub(in crate::oauth) use coordinator::OAuthQuotaService;
pub use types::{
    OAuthQuotaError, OAuthQuotaResetOutcome, OAuthQuotaSnapshot, OAuthQuotaUsdEstimate,
};
