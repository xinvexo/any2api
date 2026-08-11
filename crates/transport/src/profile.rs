use std::time::Duration;

const HTTP_2_AND_1_ALPN: &[&[u8]] = &[b"h2", b"http/1.1"];
const RESPONSE_CONTENT_CODINGS: &[&str] = &["gzip", "br", "zstd"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TransportWireProfile {
    id: &'static str,
    policy_version: u16,
    tls_policy_version: u16,
    http_version_policy_version: u16,
    pool_policy_version: u16,
    timeout_policy_version: u16,
    alpn_protocols: &'static [&'static [u8]],
    http1_enabled: bool,
    http2_enabled: bool,
    tcp_keep_alive_interval: Duration,
    pinned_tcp_nodelay: bool,
    http2_keep_alive_interval: Duration,
    http2_keep_alive_timeout: Duration,
    http2_keep_alive_while_idle: bool,
    redirects_enabled: bool,
    automatic_request_retries: bool,
    client_accept_encoding_passthrough: bool,
    response_content_codings: &'static [&'static str],
    max_response_content_coding_depth: usize,
}

pub const GENERIC_GATEWAY_TRANSPORT_PROFILE: TransportWireProfile = TransportWireProfile {
    id: "generic-rustls-hyper-v3",
    policy_version: 3,
    tls_policy_version: 1,
    http_version_policy_version: 1,
    pool_policy_version: 2,
    timeout_policy_version: 1,
    alpn_protocols: HTTP_2_AND_1_ALPN,
    http1_enabled: true,
    http2_enabled: true,
    tcp_keep_alive_interval: Duration::from_secs(30),
    pinned_tcp_nodelay: true,
    http2_keep_alive_interval: Duration::from_secs(30),
    http2_keep_alive_timeout: Duration::from_secs(10),
    http2_keep_alive_while_idle: false,
    redirects_enabled: false,
    automatic_request_retries: false,
    client_accept_encoding_passthrough: true,
    response_content_codings: RESPONSE_CONTENT_CODINGS,
    max_response_content_coding_depth: 4,
};

impl TransportWireProfile {
    #[must_use]
    pub const fn id(self) -> &'static str {
        self.id
    }

    #[must_use]
    pub const fn policy_version(self) -> u16 {
        self.policy_version
    }

    #[must_use]
    pub const fn tls_policy_version(self) -> u16 {
        self.tls_policy_version
    }

    #[must_use]
    pub const fn http_version_policy_version(self) -> u16 {
        self.http_version_policy_version
    }

    #[must_use]
    pub const fn pool_policy_version(self) -> u16 {
        self.pool_policy_version
    }

    #[must_use]
    pub const fn timeout_policy_version(self) -> u16 {
        self.timeout_policy_version
    }

    #[must_use]
    pub const fn alpn_protocols(self) -> &'static [&'static [u8]] {
        self.alpn_protocols
    }

    #[must_use]
    pub const fn http1_enabled(self) -> bool {
        self.http1_enabled
    }

    #[must_use]
    pub const fn http2_enabled(self) -> bool {
        self.http2_enabled
    }

    #[must_use]
    pub const fn tcp_keep_alive_interval(self) -> Duration {
        self.tcp_keep_alive_interval
    }

    #[must_use]
    pub const fn pinned_tcp_nodelay(self) -> bool {
        self.pinned_tcp_nodelay
    }

    #[must_use]
    pub const fn http2_keep_alive_interval(self) -> Duration {
        self.http2_keep_alive_interval
    }

    #[must_use]
    pub const fn http2_keep_alive_timeout(self) -> Duration {
        self.http2_keep_alive_timeout
    }

    #[must_use]
    pub const fn http2_keep_alive_while_idle(self) -> bool {
        self.http2_keep_alive_while_idle
    }

    #[must_use]
    pub const fn redirects_enabled(self) -> bool {
        self.redirects_enabled
    }

    #[must_use]
    pub const fn automatic_request_retries(self) -> bool {
        self.automatic_request_retries
    }

    #[must_use]
    pub const fn client_accept_encoding_passthrough(self) -> bool {
        self.client_accept_encoding_passthrough
    }

    #[must_use]
    pub const fn response_content_codings(self) -> &'static [&'static str] {
        self.response_content_codings
    }

    #[must_use]
    pub const fn max_response_content_coding_depth(self) -> usize {
        self.max_response_content_coding_depth
    }

    pub(crate) fn owned_alpn_protocols(self) -> Vec<Vec<u8>> {
        self.alpn_protocols
            .iter()
            .map(|value| value.to_vec())
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::GENERIC_GATEWAY_TRANSPORT_PROFILE as PROFILE;

    #[test]
    fn generic_gateway_v3_wire_contract_is_explicit() {
        assert_eq!(PROFILE.id(), "generic-rustls-hyper-v3");
        assert_eq!(PROFILE.policy_version(), 3);
        assert_eq!(PROFILE.tls_policy_version(), 1);
        assert_eq!(PROFILE.http_version_policy_version(), 1);
        assert_eq!(PROFILE.pool_policy_version(), 2);
        assert_eq!(PROFILE.timeout_policy_version(), 1);
        assert_eq!(PROFILE.alpn_protocols(), [b"h2".as_slice(), b"http/1.1"]);
        assert!(PROFILE.http1_enabled());
        assert!(PROFILE.http2_enabled());
        assert_eq!(PROFILE.tcp_keep_alive_interval(), Duration::from_secs(30));
        assert!(PROFILE.pinned_tcp_nodelay());
        assert_eq!(PROFILE.http2_keep_alive_interval(), Duration::from_secs(30));
        assert_eq!(PROFILE.http2_keep_alive_timeout(), Duration::from_secs(10));
        assert!(!PROFILE.http2_keep_alive_while_idle());
        assert!(!PROFILE.redirects_enabled());
        assert!(!PROFILE.automatic_request_retries());
        assert!(PROFILE.client_accept_encoding_passthrough());
        assert_eq!(PROFILE.response_content_codings(), ["gzip", "br", "zstd"]);
        assert_eq!(PROFILE.max_response_content_coding_depth(), 4);
    }
}
