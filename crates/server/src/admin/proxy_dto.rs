use any2api_domain::{
    ConfigRevision, ProxyAddress, ProxyDraft, ProxyKind, ProxyProfile, ProxyProfileId,
};
use any2api_runtime::api::PublishedSnapshot;
use serde::{Deserialize, Serialize};

use super::{error::AdminApiError, revision::parse_revision};

#[derive(Debug, Serialize)]
pub(crate) struct ProxyCollectionResponse {
    config_revision: u64,
    global_proxy_id: ProxyProfileId,
    items: Vec<ProxyResponse>,
}

impl ProxyCollectionResponse {
    pub(crate) fn from_snapshot(snapshot: &PublishedSnapshot) -> Self {
        Self {
            config_revision: snapshot.revision().get(),
            global_proxy_id: snapshot.proxies().global_proxy_id(),
            items: snapshot
                .proxies()
                .profiles()
                .iter()
                .map(ProxyResponse::from)
                .collect(),
        }
    }
}

#[derive(Debug, Serialize)]
struct ProxyResponse {
    id: ProxyProfileId,
    name: String,
    kind: ProxyKind,
    host: Option<String>,
    port: Option<u16>,
    enabled: bool,
    built_in: bool,
    config_version: u64,
}

impl From<&ProxyProfile> for ProxyResponse {
    fn from(profile: &ProxyProfile) -> Self {
        Self {
            id: profile.id(),
            name: profile.name().to_owned(),
            kind: profile.kind(),
            host: profile.address().map(|address| address.host().to_owned()),
            port: profile.address().map(ProxyAddress::port),
            enabled: profile.enabled(),
            built_in: profile.is_built_in(),
            config_version: profile.config_version(),
        }
    }
}

#[derive(Debug, Deserialize)]
pub(crate) struct ProxyWriteRequest {
    expected_revision: u64,
    name: String,
    kind: ProxyKind,
    host: String,
    port: u16,
    enabled: bool,
}

impl ProxyWriteRequest {
    pub(crate) fn into_domain(self) -> Result<(ConfigRevision, ProxyDraft), AdminApiError> {
        let revision = parse_revision(self.expected_revision)?;
        let address = ProxyAddress::new(self.host, self.port)
            .map_err(|error| AdminApiError::invalid_request(error.to_string()))?;
        let draft = ProxyDraft::new(self.name, self.kind, address, self.enabled)
            .map_err(|error| AdminApiError::invalid_request(error.to_string()))?;

        Ok((revision, draft))
    }
}
