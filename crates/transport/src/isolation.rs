use std::{
    fmt,
    sync::atomic::{AtomicU64, Ordering},
};

use any2api_domain::RoutingCredentialId;

static NEXT_EPHEMERAL_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum TransportTrafficClass {
    DataPlane,
    OAuthToken,
    OAuthQuota,
    Diagnostic,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub struct TransportIsolationKey {
    owner: TransportIsolationOwner,
    routing_generation: u64,
    authentication_version: u64,
    traffic_class: TransportTrafficClass,
}

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
enum TransportIsolationOwner {
    RoutingCredential(RoutingCredentialId),
    SharedDataPlane,
    Ephemeral(u64),
}

impl TransportIsolationKey {
    #[must_use]
    pub const fn routing_credential(
        credential_id: RoutingCredentialId,
        routing_generation: u64,
        authentication_version: u64,
        traffic_class: TransportTrafficClass,
    ) -> Self {
        Self {
            owner: TransportIsolationOwner::RoutingCredential(credential_id),
            routing_generation,
            authentication_version,
            traffic_class,
        }
    }

    /// Shared data-plane scope: all credentials multiplex the same upstream
    /// connection pool (per proxy/wire profile). Upstream prompt caches route
    /// by connection path, so splitting pools per credential breaks cache
    /// continuity across accounts.
    #[must_use]
    pub const fn shared_data_plane() -> Self {
        Self {
            owner: TransportIsolationOwner::SharedDataPlane,
            routing_generation: 1,
            authentication_version: 1,
            traffic_class: TransportTrafficClass::DataPlane,
        }
    }

    #[must_use]
    pub fn ephemeral(traffic_class: TransportTrafficClass) -> Self {
        let id = NEXT_EPHEMERAL_ID
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                current.checked_add(1)
            })
            .expect("ephemeral transport isolation identity exhausted u64");
        Self {
            owner: TransportIsolationOwner::Ephemeral(id),
            routing_generation: 1,
            authentication_version: 1,
            traffic_class,
        }
    }

    #[must_use]
    pub const fn traffic_class(self) -> TransportTrafficClass {
        self.traffic_class
    }

    pub(crate) const fn routing_generation(self) -> u64 {
        self.routing_generation
    }

    pub(crate) const fn authentication_version(self) -> u64 {
        self.authentication_version
    }

    pub(crate) const fn is_ephemeral(self) -> bool {
        matches!(self.owner, TransportIsolationOwner::Ephemeral(_))
    }

    pub(crate) fn retires(self, cached: Self) -> bool {
        match (self.owner, cached.owner) {
            (
                TransportIsolationOwner::RoutingCredential(current),
                TransportIsolationOwner::RoutingCredential(previous),
            ) if current == previous => {
                (self.routing_generation, self.authentication_version)
                    > (cached.routing_generation, cached.authentication_version)
            }
            _ => false,
        }
    }
}

impl fmt::Debug for TransportIsolationKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("TransportIsolationKey")
            .field(
                "owner_kind",
                &match self.owner {
                    TransportIsolationOwner::RoutingCredential(_) => "routing_credential",
                    TransportIsolationOwner::SharedDataPlane => "shared_data_plane",
                    TransportIsolationOwner::Ephemeral(_) => "ephemeral",
                },
            )
            .field("routing_generation", &self.routing_generation)
            .field("authentication_version", &self.authentication_version)
            .field("traffic_class", &self.traffic_class)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use any2api_domain::{CredentialId, OAuthAccountId, RoutingCredentialId};

    use super::{TransportIsolationKey, TransportTrafficClass};

    #[test]
    fn credential_source_generation_and_traffic_class_are_part_of_identity() {
        let provider = RoutingCredentialId::provider_credential(CredentialId::new());
        let oauth =
            RoutingCredentialId::oauth_account(OAuthAccountId::from_uuid(provider.source_uuid()));
        let base = TransportIsolationKey::routing_credential(
            provider,
            1,
            1,
            TransportTrafficClass::DataPlane,
        );

        assert_ne!(
            base,
            TransportIsolationKey::routing_credential(
                oauth,
                1,
                1,
                TransportTrafficClass::DataPlane,
            )
        );
        assert_ne!(
            base,
            TransportIsolationKey::routing_credential(
                provider,
                1,
                2,
                TransportTrafficClass::DataPlane,
            )
        );
        assert_ne!(
            base,
            TransportIsolationKey::routing_credential(
                provider,
                1,
                1,
                TransportTrafficClass::OAuthQuota,
            )
        );

        let rotated = TransportIsolationKey::routing_credential(
            provider,
            2,
            2,
            TransportTrafficClass::DataPlane,
        );
        assert!(rotated.retires(base));
        assert!(!base.retires(rotated));
    }

    #[test]
    fn ephemeral_identities_never_share_a_scope() {
        assert_ne!(
            TransportIsolationKey::ephemeral(TransportTrafficClass::Diagnostic),
            TransportIsolationKey::ephemeral(TransportTrafficClass::Diagnostic),
        );
    }
}
