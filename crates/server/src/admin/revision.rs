use any2api_domain::ConfigRevision;
use serde::Deserialize;

use super::error::AdminApiError;

#[derive(Debug, Deserialize)]
pub(crate) struct ExpectedRevisionRequest {
    expected_revision: u64,
}

impl ExpectedRevisionRequest {
    pub(crate) fn revision(self) -> Result<ConfigRevision, AdminApiError> {
        parse_revision(self.expected_revision)
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ExpectedRevisionQuery {
    expected_revision: u64,
}

impl ExpectedRevisionQuery {
    pub(crate) fn revision(self) -> Result<ConfigRevision, AdminApiError> {
        parse_revision(self.expected_revision)
    }
}

pub(crate) fn parse_revision(value: u64) -> Result<ConfigRevision, AdminApiError> {
    ConfigRevision::new(value)
        .map_err(|_| AdminApiError::invalid_request("expected_revision is invalid"))
}
