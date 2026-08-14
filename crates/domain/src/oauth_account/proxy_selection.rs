use crate::ProxyProfileId;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthProxySelection {
    Global,
    Profile(ProxyProfileId),
}

impl OAuthProxySelection {
    #[must_use]
    pub const fn from_profile_id(profile_id: Option<ProxyProfileId>) -> Self {
        match profile_id {
            Some(profile_id) => Self::Profile(profile_id),
            None => Self::Global,
        }
    }

    #[must_use]
    pub const fn profile_id(self) -> Option<ProxyProfileId> {
        match self {
            Self::Global => None,
            Self::Profile(profile_id) => Some(profile_id),
        }
    }
}
