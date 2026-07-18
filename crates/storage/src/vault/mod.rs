mod cipher;
mod context;
mod envelope;
mod error;
mod master_key;
mod metadata;

pub use cipher::{SecretBytes, SecretVault};
pub use context::SecretContext;
pub use envelope::{SecretAlgorithm, SecretEnvelope};
pub use error::SecretVaultError;

pub(crate) use metadata::initialize_vault;

#[cfg(test)]
mod tests;
