mod buffered;
mod failure;
mod prepared;
mod streaming;

pub(super) use buffered::execute_buffered_attempt;
pub(super) use failure::AttemptFailure;
pub(super) use streaming::execute_stream_attempt;

use any2api_protocol::api::ProtocolRegistry;
use any2api_provider::api::ProviderRegistry;
use any2api_transport::api::TransportManager;
use tokio::time::Instant;

use crate::oauth::OAuthQuotaActivity;
use crate::{
    configuration::PublishedSnapshot,
    routing::{CacheLocalityKey, RouteCandidate},
};

#[derive(Clone, Copy)]
pub(super) struct UpstreamServices<'a> {
    pub(super) snapshot: &'a PublishedSnapshot,
    pub(super) protocols: &'a ProtocolRegistry,
    pub(super) providers: &'a ProviderRegistry,
    pub(super) transport: &'a dyn TransportManager,
    pub(super) oauth_quota_activity: Option<&'a OAuthQuotaActivity>,
    pub(super) cache_locality_key: Option<CacheLocalityKey>,
    pub(super) attempt_deadline: Instant,
}

fn forget_cache_locality(services: &UpstreamServices<'_>, candidate: &RouteCandidate) {
    if let Some(key) = services.cache_locality_key {
        services
            .snapshot
            .cache_locality_registry()
            .forget_candidate(key, candidate);
    }
}
