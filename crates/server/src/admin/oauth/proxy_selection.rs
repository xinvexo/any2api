use any2api_domain::{OAuthProxySelection, ProxyProfileId};
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(tag = "mode", rename_all = "snake_case", deny_unknown_fields)]
pub(super) enum OAuthProxySelectionDto {
    Global,
    Profile { proxy_profile_id: ProxyProfileId },
}

impl From<OAuthProxySelectionDto> for OAuthProxySelection {
    fn from(value: OAuthProxySelectionDto) -> Self {
        match value {
            OAuthProxySelectionDto::Global => Self::Global,
            OAuthProxySelectionDto::Profile { proxy_profile_id } => Self::Profile(proxy_profile_id),
        }
    }
}

impl From<OAuthProxySelection> for OAuthProxySelectionDto {
    fn from(value: OAuthProxySelection) -> Self {
        match value {
            OAuthProxySelection::Global => Self::Global,
            OAuthProxySelection::Profile(proxy_profile_id) => Self::Profile { proxy_profile_id },
        }
    }
}
