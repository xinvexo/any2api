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

use crate::published_snapshot::PublishedSnapshot;

#[derive(Clone, Copy)]
pub(super) struct UpstreamServices<'a> {
    pub(super) snapshot: &'a PublishedSnapshot,
    pub(super) protocols: &'a ProtocolRegistry,
    pub(super) providers: &'a ProviderRegistry,
    pub(super) transport: &'a dyn TransportManager,
}
