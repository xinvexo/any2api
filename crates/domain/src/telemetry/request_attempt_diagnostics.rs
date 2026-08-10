use crate::ProxyKind;

pub const MAX_TRANSPORT_WIRE_PROFILE_ID_CHARS: usize = 64;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestTransportResolverMode {
    System,
    ProxyRemote,
    LocalCached,
}

impl RequestTransportResolverMode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::System => "system",
            Self::ProxyRemote => "proxy_remote",
            Self::LocalCached => "local_cached",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "system" => Some(Self::System),
            "proxy_remote" => Some(Self::ProxyRemote),
            "local_cached" => Some(Self::LocalCached),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestTransportTrafficClass {
    DataPlane,
    OAuthToken,
    OAuthQuota,
    Diagnostic,
}

impl RequestTransportTrafficClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DataPlane => "data_plane",
            Self::OAuthToken => "oauth_token",
            Self::OAuthQuota => "oauth_quota",
            Self::Diagnostic => "diagnostic",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "data_plane" => Some(Self::DataPlane),
            "oauth_token" => Some(Self::OAuthToken),
            "oauth_quota" => Some(Self::OAuthQuota),
            "diagnostic" => Some(Self::Diagnostic),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestAttemptTransport {
    pub wire_profile_id: String,
    pub wire_profile_version: u16,
    pub timeout_policy_version: u16,
    pub resolver_mode: RequestTransportResolverMode,
    pub proxy_kind: ProxyKind,
    pub connect_timeout_ms: u64,
    pub read_timeout_ms: u64,
    pub pool_idle_timeout_ms: u64,
    pub routing_generation: u64,
    pub authentication_version: u64,
    pub traffic_class: RequestTransportTrafficClass,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct RequestAttemptStreamTiming {
    pub first_upstream_frame_ms: Option<u64>,
    pub stream_commit_ms: Option<u64>,
    pub first_downstream_byte_ms: Option<u64>,
    pub stream_cancel_ms: Option<u64>,
}

impl RequestAttemptStreamTiming {
    #[must_use]
    pub const fn is_empty(self) -> bool {
        self.first_upstream_frame_ms.is_none()
            && self.stream_commit_ms.is_none()
            && self.first_downstream_byte_ms.is_none()
            && self.stream_cancel_ms.is_none()
    }
}
