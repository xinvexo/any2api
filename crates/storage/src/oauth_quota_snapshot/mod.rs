mod repository;

#[cfg(test)]
mod tests;

pub use repository::{
    MAX_OAUTH_QUOTA_SNAPSHOT_BYTES, OAUTH_QUOTA_SNAPSHOT_SCHEMA_VERSION,
    OAuthQuotaSnapshotRepository, StoredOAuthQuotaSnapshot,
};
