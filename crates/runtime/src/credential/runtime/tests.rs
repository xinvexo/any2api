use std::{sync::Arc, time::Duration};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, ProtocolDialect, ProviderCredential,
    ProviderCredentialConfiguration, ProviderCredentialDraft, ProviderEndpoint,
    ProviderEndpointConfiguration, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyConfiguration, ProxyProfileId, RequestsPerMinute,
};
use any2api_transport::api::TransportTrafficClass;
use tokio::sync::{mpsc, watch};

use crate::{
    credential::{CredentialAuthMaterials, CredentialFilterKind, CredentialRuntimeBindings},
    registry::RuntimeRegistry,
    routing::{SelectAndReserveResult, select_and_try_reserve},
};

mod fixed_waiters;

#[test]
fn balancing_counters_are_stable_with_the_credential_handle() {
    let registry = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let initial = reconcile(
        &registry,
        fixture.configuration(Some(2), 1, 1),
        "sk-balancing-test",
    );
    let binding = initial.as_slice()[0].clone();
    binding.record_selection();
    binding.record_filter(CredentialFilterKind::RateLimit);
    binding.record_filter(CredentialFilterKind::CredentialHealth);
    binding.record_filter(CredentialFilterKind::EndpointHealth);
    binding.record_filter(CredentialFilterKind::ProxyHealth);

    let updated = reconcile(
        &registry,
        fixture.configuration(Some(3), 1, 1),
        "sk-balancing-test",
    );
    let counters = updated.as_slice()[0].balancing_counters();
    assert_eq!(counters.selected(), 1);
    assert_eq!(counters.filtered_rate_limit(), 1);
    assert_eq!(counters.filtered_credential_health(), 1);
    assert_eq!(counters.filtered_endpoint_health(), 1);
    assert_eq!(counters.filtered_proxy_health(), 1);

    let restarted = RuntimeRegistry::new();
    let fresh = reconcile(
        &restarted,
        fixture.configuration(Some(3), 1, 1),
        "sk-balancing-test",
    );
    assert_eq!(fresh.as_slice()[0].balancing_counters(), Default::default());
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_reservations_never_exceed_the_configured_rpm() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let bindings = reconcile(
        &runtime,
        fixture.configuration(Some(4), 1, 1),
        "sk-rate-test",
    );
    let binding = bindings.as_slice()[0].clone();
    let (result_tx, mut result_rx) = mpsc::unbounded_channel();
    let (release_tx, release_rx) = watch::channel(false);
    let mut tasks = Vec::new();

    for _ in 0..64 {
        let binding = binding.clone();
        let result_tx = result_tx.clone();
        let mut release_rx = release_rx.clone();
        tasks.push(tokio::spawn(async move {
            let permit = binding.try_reserve().ok();
            result_tx
                .send(permit.is_some())
                .expect("result receiver remains open");
            if let Some(permit) = permit {
                while !*release_rx.borrow() {
                    release_rx
                        .changed()
                        .await
                        .expect("release sender remains open");
                }
                drop(permit);
            }
        }));
    }
    drop(result_tx);

    let mut reserved = 0;
    for _ in 0..64 {
        reserved += usize::from(result_rx.recv().await.expect("task result"));
    }
    assert_eq!(reserved, 4);
    assert_eq!(binding.in_flight(), 4);
    assert_eq!(binding.rate_snapshot().requests_in_window(), 4);

    release_tx
        .send(true)
        .expect("release receivers remain open");
    for task in tasks {
        task.await.expect("reservation task");
    }
    assert_eq!(binding.in_flight(), 0);
    assert_eq!(binding.rate_snapshot().requests_in_window(), 4);
    assert_eq!(runtime.scheduler_epoch(), 0);
}

#[tokio::test(start_paused = true)]
async fn reservations_expire_after_an_exact_rolling_minute() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let bindings = reconcile(
        &runtime,
        fixture.configuration(Some(1), 1, 1),
        "sk-expiry-test",
    );
    let binding = bindings.as_slice()[0].clone();
    drop(binding.try_reserve().expect("first reservation"));
    assert!(binding.try_reserve().is_err());

    tokio::time::advance(Duration::from_secs(59)).await;
    assert!(binding.try_reserve().is_err());
    tokio::time::advance(Duration::from_secs(1)).await;
    assert!(binding.try_reserve().is_ok());
}

#[test]
fn rpm_reservation_starts_after_waiting_for_mutable_state() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let bindings = reconcile(
        &runtime,
        fixture.configuration(Some(1), 1, 1),
        "sk-lock-timing-test",
    );
    let binding = bindings.as_slice()[0].clone();
    let held_state = binding.handle.hold_mutable_lock_for_test();
    let (started_tx, started_rx) = std::sync::mpsc::sync_channel(0);
    let contender = binding.clone();
    let reservation = std::thread::spawn(move || {
        started_tx.send(()).expect("test receiver remains open");
        contender.try_reserve()
    });

    started_rx.recv().expect("reservation thread started");
    std::thread::sleep(Duration::from_millis(50));
    let unlocked_at = tokio::time::Instant::now();
    drop(held_state);

    let permit = reservation
        .join()
        .expect("reservation thread")
        .expect("reservation after lock release");
    let retry_at = binding
        .rate_snapshot()
        .retry_at()
        .expect("finite full window");
    assert!(retry_at >= unlocked_at + Duration::from_secs(60));
    drop(permit);
}

#[test]
fn finite_limit_changes_preserve_the_current_window() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let initial = reconcile(
        &runtime,
        fixture.configuration(Some(3), 1, 1),
        "sk-limit-change",
    );
    let binding = initial.as_slice()[0].clone();
    let held = (0..3)
        .map(|_| binding.try_reserve().expect("initial reservation"))
        .collect::<Vec<_>>();

    let lowered = reconcile(
        &runtime,
        fixture.configuration(Some(1), 1, 1),
        "sk-limit-change",
    );
    let lowered = &lowered.as_slice()[0];
    assert_eq!(lowered.in_flight(), 3);
    assert_eq!(lowered.rate_snapshot().requests_per_minute(), Some(1));
    assert_eq!(lowered.rate_snapshot().requests_in_window(), 3);
    assert!(lowered.try_reserve().is_err());

    let raised = reconcile(
        &runtime,
        fixture.configuration(Some(4), 1, 1),
        "sk-limit-change",
    );
    let raised = &raised.as_slice()[0];
    assert!(raised.try_reserve().is_ok());
    assert!(raised.try_reserve().is_err());
    drop(held);
}

#[test]
fn unlimited_revision_does_not_clear_the_shared_finite_window() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let initial = reconcile(
        &runtime,
        fixture.configuration(Some(1), 1, 1),
        "sk-unlimited",
    );
    let binding = initial.as_slice()[0].clone();
    let first = binding.try_reserve().expect("limited reservation");

    let unlimited = reconcile(&runtime, fixture.configuration(None, 1, 1), "sk-unlimited");
    let unlimited = unlimited.as_slice()[0].clone();
    let second = unlimited.try_reserve().expect("unlimited reservation");
    let third = unlimited.try_reserve().expect("unlimited reservation");
    assert_eq!(unlimited.in_flight(), 3);
    assert_eq!(unlimited.rate_snapshot().requests_per_minute(), None);
    assert_eq!(unlimited.rate_snapshot().requests_in_window(), 0);
    assert_eq!(binding.rate_snapshot().requests_per_minute(), Some(1));
    assert_eq!(binding.rate_snapshot().requests_in_window(), 1);
    assert!(binding.try_reserve().is_err());

    let limited_again = reconcile(
        &runtime,
        fixture.configuration(Some(1), 1, 1),
        "sk-unlimited",
    );
    let limited_again = &limited_again.as_slice()[0];
    assert!(limited_again.try_reserve().is_err());
    drop((first, second, third));
}

#[test]
fn generation_changes_are_pinned_without_resetting_the_rate_window() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let initial = reconcile(
        &runtime,
        fixture.configuration(Some(2), 1, 1),
        "sk-old-generation",
    );
    let old_binding = initial.as_slice()[0].clone();
    let old_permit = old_binding.try_reserve().expect("old generation permit");
    let old_data_isolation = old_permit.transport_isolation(TransportTrafficClass::DataPlane);
    assert_ne!(
        old_data_isolation,
        old_permit.transport_isolation(TransportTrafficClass::Diagnostic)
    );

    let rotated = reconcile(
        &runtime,
        fixture.configuration(Some(2), 2, 2),
        "sk-new-generation",
    );
    let new_binding = rotated.as_slice()[0].clone();
    let new_data_isolation = new_binding.transport_isolation(TransportTrafficClass::DataPlane);
    assert_ne!(old_data_isolation, new_data_isolation);
    assert_eq!(old_permit.generation().routing_generation(), 1);
    assert_eq!(new_binding.generation().routing_generation(), 2);
    assert_eq!(new_binding.generation().authentication_version(), 2);
    assert_eq!(
        old_permit
            .generation()
            .provider_secret()
            .expect("Provider API Key generation")
            .expose(),
        "sk-old-generation"
    );
    assert_eq!(
        new_binding
            .generation()
            .provider_secret()
            .expect("Provider API Key generation")
            .expose(),
        "sk-new-generation"
    );
    assert!(!format!("{old_permit:?}").contains("sk-old-generation"));
    assert!(!format!("{:?}", new_binding.generation()).contains("sk-new-generation"));
    assert!(!Arc::ptr_eq(
        old_permit.generation(),
        new_binding.generation()
    ));
    assert_eq!(new_binding.in_flight(), 1);
    assert_eq!(new_binding.rate_snapshot().requests_in_window(), 1);

    let new_permit = new_binding.try_reserve().expect("new generation permit");
    assert_eq!(new_permit.generation().routing_generation(), 2);
    assert_eq!(
        new_permit.transport_isolation(TransportTrafficClass::DataPlane),
        new_data_isolation
    );
    assert!(new_binding.try_reserve().is_err());
    drop((old_permit, new_permit));
}

#[test]
fn selector_skips_rate_limited_credentials_and_rotates_available_ties() {
    let first_runtime = RuntimeRegistry::new();
    let first_fixture = CredentialFixture::new();
    let first = reconcile(
        &first_runtime,
        first_fixture.configuration(Some(1), 1, 1),
        "sk-first-selector",
    )
    .as_slice()[0]
        .clone();
    drop(first.try_reserve().expect("exhaust first credential"));

    let second_runtime = RuntimeRegistry::new();
    let second_fixture = CredentialFixture::new();
    let second = reconcile(
        &second_runtime,
        second_fixture.configuration(Some(2), 1, 1),
        "sk-second-selector",
    )
    .as_slice()[0]
        .clone();
    let selected = select_and_try_reserve(&[first.clone(), second.clone()], 0);
    let SelectAndReserveResult::Reserved(selected) = selected else {
        panic!("an available credential must be selected");
    };
    assert_eq!(selected.credential_id(), second.credential_id());
    drop(selected);

    let tie_runtime = RuntimeRegistry::new();
    let tie_fixture = CredentialFixture::new();
    let tie = reconcile(
        &tie_runtime,
        tie_fixture.configuration(None, 1, 1),
        "sk-tie-selector",
    )
    .as_slice()[0]
        .clone();
    let selected = select_and_try_reserve(&[second, tie.clone()], 1);
    let SelectAndReserveResult::Reserved(selected) = selected else {
        panic!("an unlimited credential must be selected");
    };
    assert_eq!(selected.credential_id(), tie.credential_id());
}

fn reconcile(
    runtime: &RuntimeRegistry,
    configuration: ProviderCredentialConfiguration,
    secret: &str,
) -> CredentialRuntimeBindings {
    let auth_materials =
        CredentialAuthMaterials::for_configuration(&configuration, |_| secret.to_owned());
    runtime.reconcile_provider_configuration_for_test(&configuration, auth_materials)
}

struct CredentialFixture {
    credential_id: CredentialId,
    endpoint_id: ProviderEndpointId,
    endpoints: ProviderEndpointConfiguration,
    proxies: ProxyConfiguration,
}

impl CredentialFixture {
    fn new() -> Self {
        let endpoint_id = ProviderEndpointId::new();
        let endpoint = ProviderEndpoint::create(
            endpoint_id,
            ProviderEndpointDraft::new(
                "Codex",
                ProviderKind::Codex,
                "https://api.example.com",
                ProtocolDialect::OpenAiResponses,
                None,
                true,
            )
            .expect("endpoint draft"),
        )
        .expect("endpoint");
        Self {
            credential_id: CredentialId::new(),
            endpoint_id,
            endpoints: ProviderEndpointConfiguration::new(vec![endpoint])
                .expect("endpoint configuration"),
            proxies: ProxyConfiguration::initial(),
        }
    }

    fn configuration(
        &self,
        requests_per_minute: Option<u32>,
        credential_generation: u64,
        secret_version: u64,
    ) -> ProviderCredentialConfiguration {
        let requests_per_minute = requests_per_minute
            .map(|value| RequestsPerMinute::new(value).expect("valid requests per minute"));
        let draft = ProviderCredentialDraft::new(
            "Primary",
            CredentialKind::ApiKey,
            ProxyProfileId::DIRECT,
            requests_per_minute,
            true,
        )
        .expect("credential draft");
        let fingerprint = CredentialSecretFingerprint::new([0x5a; 32], Some("test".to_owned()))
            .expect("fingerprint");
        let credential = ProviderCredential::restore(
            self.credential_id,
            self.endpoint_id,
            draft,
            fingerprint,
            secret_version,
            credential_generation,
            credential_generation,
            Vec::new(),
        )
        .expect("credential");
        ProviderCredentialConfiguration::new(vec![credential], &self.endpoints, &self.proxies)
            .expect("credential configuration")
    }
}
