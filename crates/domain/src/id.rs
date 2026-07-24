use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};
use uuid::Uuid;

macro_rules! define_id {
    ($name:ident) => {
        #[derive(
            Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize,
        )]
        #[serde(transparent)]
        pub struct $name(Uuid);

        impl $name {
            #[must_use]
            pub fn new() -> Self {
                Self(Uuid::new_v4())
            }

            #[must_use]
            pub const fn from_uuid(value: Uuid) -> Self {
                Self(value)
            }

            #[must_use]
            pub const fn as_uuid(&self) -> &Uuid {
                &self.0
            }
        }

        impl Default for $name {
            fn default() -> Self {
                Self::new()
            }
        }

        impl fmt::Display for $name {
            fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
                self.0.fmt(formatter)
            }
        }

        impl FromStr for $name {
            type Err = uuid::Error;

            fn from_str(value: &str) -> Result<Self, Self::Err> {
                Uuid::parse_str(value).map(Self)
            }
        }
    };
}

define_id!(RequestId);
define_id!(CredentialId);
define_id!(ProviderEndpointId);
define_id!(ProxyProfileId);
define_id!(ModelRouteId);
define_id!(RouteTargetId);
define_id!(GatewayApiKeyId);
define_id!(OAuthAccountId);

impl ProxyProfileId {
    pub const DIRECT: Self = Self(Uuid::nil());
}

#[cfg(test)]
mod tests {
    use super::RequestId;

    #[test]
    fn id_round_trips_through_text() {
        let id = RequestId::new();
        let decoded = id
            .to_string()
            .parse::<RequestId>()
            .expect("valid request id");

        assert_eq!(decoded, id);
    }
}
