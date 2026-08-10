use std::time::Instant;

use any2api_domain::{
    RequestAttemptStreamTiming, RequestAttemptTransport, RequestTransportResolverMode,
    RequestTransportTrafficClass,
};
use any2api_transport::api::{
    TransportRequestDiagnostics, TransportResolverMode, TransportTrafficClass,
};

use super::request::duration_ms;

#[derive(Default)]
pub(super) struct AttemptDiagnostics {
    transport: Option<RequestAttemptTransport>,
    stream_timing: RequestAttemptStreamTiming,
}

impl AttemptDiagnostics {
    #[cfg(test)]
    pub(super) const fn stream_timing(&self) -> RequestAttemptStreamTiming {
        self.stream_timing
    }

    pub(super) fn observe_transport(&mut self, diagnostics: TransportRequestDiagnostics) {
        self.transport
            .get_or_insert_with(|| RequestAttemptTransport {
                wire_profile_id: diagnostics.wire_profile_id().to_owned(),
                wire_profile_version: diagnostics.wire_profile_version(),
                timeout_policy_version: diagnostics.timeout_policy_version(),
                resolver_mode: resolver_mode(diagnostics.resolver_mode()),
                proxy_kind: diagnostics.proxy_kind(),
                connect_timeout_ms: duration_ms(diagnostics.connect_timeout()),
                read_timeout_ms: duration_ms(diagnostics.read_timeout()),
                pool_idle_timeout_ms: duration_ms(diagnostics.pool_idle_timeout()),
                routing_generation: diagnostics.routing_generation(),
                authentication_version: diagnostics.authentication_version(),
                traffic_class: traffic_class(diagnostics.traffic_class()),
            });
    }

    pub(super) fn observe_first_upstream_frame(&mut self, started_at: Instant) {
        observe_once(&mut self.stream_timing.first_upstream_frame_ms, started_at);
    }

    pub(super) fn observe_stream_commit(&mut self, started_at: Instant) {
        observe_once(&mut self.stream_timing.stream_commit_ms, started_at);
    }

    pub(super) fn observe_first_downstream_byte(&mut self, started_at: Instant) {
        observe_once(&mut self.stream_timing.first_downstream_byte_ms, started_at);
    }

    pub(super) fn observe_stream_cancel(&mut self, started_at: Instant) {
        observe_once(&mut self.stream_timing.stream_cancel_ms, started_at);
    }

    pub(super) fn take(
        &mut self,
    ) -> (
        Option<RequestAttemptTransport>,
        Option<RequestAttemptStreamTiming>,
    ) {
        let transport = self.transport.take();
        let stream_timing = std::mem::take(&mut self.stream_timing);
        (
            transport,
            (!stream_timing.is_empty()).then_some(stream_timing),
        )
    }
}

fn observe_once(value: &mut Option<u64>, started_at: Instant) {
    if value.is_none() {
        *value = Some(duration_ms(started_at.elapsed()));
    }
}

const fn resolver_mode(value: TransportResolverMode) -> RequestTransportResolverMode {
    match value {
        TransportResolverMode::System => RequestTransportResolverMode::System,
        TransportResolverMode::ProxyRemote => RequestTransportResolverMode::ProxyRemote,
        TransportResolverMode::LocalCached => RequestTransportResolverMode::LocalCached,
    }
}

const fn traffic_class(value: TransportTrafficClass) -> RequestTransportTrafficClass {
    match value {
        TransportTrafficClass::DataPlane => RequestTransportTrafficClass::DataPlane,
        TransportTrafficClass::OAuthToken => RequestTransportTrafficClass::OAuthToken,
        TransportTrafficClass::OAuthQuota => RequestTransportTrafficClass::OAuthQuota,
        TransportTrafficClass::Diagnostic => RequestTransportTrafficClass::Diagnostic,
    }
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use super::AttemptDiagnostics;

    #[test]
    fn stream_milestones_are_first_write_wins_and_cancel_is_independent() {
        let started_at = Instant::now() - Duration::from_millis(20);
        let mut diagnostics = AttemptDiagnostics::default();
        diagnostics.observe_first_upstream_frame(started_at);
        let first_frame = diagnostics.stream_timing.first_upstream_frame_ms;
        diagnostics.observe_first_upstream_frame(Instant::now());
        diagnostics.observe_stream_commit(started_at);
        diagnostics.observe_first_downstream_byte(started_at);
        diagnostics.observe_stream_cancel(started_at);

        let (_, timing) = diagnostics.take();
        let timing = timing.expect("stream timing");
        assert_eq!(timing.first_upstream_frame_ms, first_frame);
        assert!(timing.stream_commit_ms.is_some());
        assert!(timing.first_downstream_byte_ms.is_some());
        assert!(timing.stream_cancel_ms.is_some());
    }

    #[test]
    fn untouched_diagnostics_remain_absent() {
        let mut diagnostics = AttemptDiagnostics::default();
        assert_eq!(diagnostics.take(), (None, None));
    }
}
