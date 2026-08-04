mod load;
mod model;
mod mutation;
mod readback;
mod repository;
mod revision;
mod sqlite_repository;
mod transaction;
mod write_consistency;

#[cfg(test)]
mod readback_tests;
#[cfg(test)]
mod test_support;

pub use model::{StoredConfiguration, StoredConfigurationParts};
pub use mutation::ConfigurationMutation;
pub use repository::{ConfigurationRepository, ConfigurationTransactionRepository};
pub use transaction::{ConfigurationCandidateCompiler, ConfigurationTransactionOutcome};

#[cfg(test)]
pub(crate) use test_support::commit_configuration;

pub(crate) use load::load_configuration_from;
pub(crate) use readback::{
    readback_gateway_api_key_mutation, readback_oauth_account_mutation,
    readback_provider_credential_mutation, readback_provider_endpoint_mutation,
    readback_proxy_mutation, readback_setting_mutation,
};
pub(crate) use revision::{bump_revision, load_revision_from};
pub(crate) use write_consistency::ensure_write_matches;
