use std::{
    collections::{HashMap, HashSet},
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
};

use any2api_domain::{ProviderEndpointId, ProxyProfileId, RoutingCredentialId};
use thiserror::Error;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub(crate) struct RouteAdmissionIdentity {
    credential_id: RoutingCredentialId,
    routing_generation: u64,
    endpoint_id: ProviderEndpointId,
    endpoint_config_version: u64,
    proxy_id: ProxyProfileId,
    proxy_config_version: u64,
}

impl RouteAdmissionIdentity {
    pub(crate) const fn new(
        credential_id: RoutingCredentialId,
        routing_generation: u64,
        endpoint_id: ProviderEndpointId,
        endpoint_config_version: u64,
        proxy_id: ProxyProfileId,
        proxy_config_version: u64,
    ) -> Self {
        Self {
            credential_id,
            routing_generation,
            endpoint_id,
            endpoint_config_version,
            proxy_id,
            proxy_config_version,
        }
    }
}

#[derive(Debug)]
pub(crate) struct RouteAdmission {
    identity: RouteAdmissionIdentity,
    incarnation: u64,
    active: AtomicBool,
}

impl RouteAdmission {
    fn new(identity: RouteAdmissionIdentity, incarnation: u64) -> Arc<Self> {
        Arc::new(Self {
            identity,
            incarnation,
            active: AtomicBool::new(true),
        })
    }

    pub(crate) fn is_active(&self) -> bool {
        self.active.load(Ordering::Acquire)
    }

    pub(crate) fn try_start(self: &Arc<Self>) -> Result<AttemptStartPermit, AttemptStartRejected> {
        if self
            .active
            .compare_exchange(true, true, Ordering::Acquire, Ordering::Acquire)
            .is_err()
        {
            return Err(AttemptStartRejected);
        }
        Ok(AttemptStartPermit {
            _admission: Arc::clone(self),
        })
    }

    fn revoke(&self) {
        self.active.swap(false, Ordering::AcqRel);
    }

    #[cfg(test)]
    pub(crate) fn active_for_test(identity: RouteAdmissionIdentity) -> Arc<Self> {
        Self::new(identity, 1)
    }

    #[cfg(test)]
    pub(crate) fn revoke_for_test(&self) {
        self.revoke();
    }
}

/// Returned when a route was withdrawn before an upstream Attempt started.
#[derive(Clone, Copy, Debug, Eq, Error, PartialEq)]
#[error("route admission is no longer active")]
pub struct AttemptStartRejected;

#[derive(Debug)]
pub(crate) struct AttemptStartPermit {
    _admission: Arc<RouteAdmission>,
}

#[derive(Debug)]
pub(crate) struct RouteAdmissionRegistry {
    state: Mutex<RouteAdmissionRegistryState>,
}

#[derive(Debug)]
struct RouteAdmissionRegistryState {
    next_incarnation: u64,
    by_identity: HashMap<RouteAdmissionIdentity, Weak<RouteAdmission>>,
}

impl Default for RouteAdmissionRegistry {
    fn default() -> Self {
        Self {
            state: Mutex::new(RouteAdmissionRegistryState {
                next_incarnation: 1,
                by_identity: HashMap::new(),
            }),
        }
    }
}

impl RouteAdmissionRegistry {
    pub(crate) fn prepare(&self, identity: RouteAdmissionIdentity) -> Arc<RouteAdmission> {
        let mut state = self
            .state
            .lock()
            .expect("route admission registry lock poisoned");
        if let Some(current) = state.by_identity.get(&identity).and_then(Weak::upgrade)
            && current.is_active()
        {
            return current;
        }

        let incarnation = state.next_incarnation;
        state.next_incarnation = state
            .next_incarnation
            .checked_add(1)
            .expect("route admission incarnation overflowed u64");
        let admission = RouteAdmission::new(identity, incarnation);
        state
            .by_identity
            .insert(identity, Arc::downgrade(&admission));
        admission
    }

    pub(crate) fn publish_current<'a>(
        &self,
        current: impl IntoIterator<Item = &'a Arc<RouteAdmission>>,
    ) {
        let current = current
            .into_iter()
            .map(|admission| admission.incarnation)
            .collect::<HashSet<_>>();
        let mut state = self
            .state
            .lock()
            .expect("route admission registry lock poisoned");
        state.by_identity.retain(|identity, admission| {
            let Some(admission) = admission.upgrade() else {
                return false;
            };
            debug_assert_eq!(*identity, admission.identity);
            if current.contains(&admission.incarnation) {
                true
            } else {
                admission.revoke();
                false
            }
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn identity() -> RouteAdmissionIdentity {
        RouteAdmissionIdentity::new(
            RoutingCredentialId::provider_credential(any2api_domain::CredentialId::new()),
            7,
            ProviderEndpointId::new(),
            3,
            ProxyProfileId::DIRECT,
            5,
        )
    }

    #[test]
    fn current_activation_is_reused_but_revoked_incarnation_never_reactivates() {
        let registry = RouteAdmissionRegistry::default();
        let identity = identity();
        let first = registry.prepare(identity);
        registry.publish_current([&first]);
        assert!(Arc::ptr_eq(&first, &registry.prepare(identity)));

        registry.publish_current(std::iter::empty());
        assert!(!first.is_active());
        assert!(matches!(first.try_start(), Err(AttemptStartRejected)));

        let reenabled = registry.prepare(identity);
        assert!(reenabled.is_active());
        assert!(!Arc::ptr_eq(&first, &reenabled));
        assert_ne!(first.incarnation, reenabled.incarnation);
    }

    #[test]
    fn permit_acquired_before_revocation_remains_a_started_attempt() {
        let registry = RouteAdmissionRegistry::default();
        let admission = registry.prepare(identity());
        let started = admission.try_start().expect("active route starts");

        registry.publish_current(std::iter::empty());

        assert!(!admission.is_active());
        assert!(matches!(admission.try_start(), Err(AttemptStartRejected)));
        drop(started);
    }

    #[test]
    fn unpublished_prepared_activation_is_revoked_at_the_next_cutover() {
        let registry = RouteAdmissionRegistry::default();
        let prepared = registry.prepare(identity());

        registry.publish_current(std::iter::empty());

        assert!(!prepared.is_active());
    }
}
