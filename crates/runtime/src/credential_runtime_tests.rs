use std::sync::Arc;

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, MaxConcurrency, ProtocolDialect,
    ProviderCredential, ProviderCredentialConfiguration, ProviderCredentialDraft, ProviderEndpoint,
    ProviderEndpointConfiguration, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyConfiguration, ProxyProfileId,
};
use tokio::sync::{mpsc, watch};

use crate::{
    registry::RuntimeRegistry,
    scheduler::{SelectAndAcquireResult, select_and_try_acquire},
};

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_acquires_never_exceed_the_configured_limit() {
    let runtime = RuntimeRegistry::new();
    let mut scheduler_epoch = runtime.subscribe_scheduler_epoch();
    let fixture = CredentialFixture::new();
    let bindings = runtime.reconcile_configuration(&fixture.configuration(4, 1, 1));
    let binding = bindings.as_slice()[0].clone();
    let (result_tx, mut result_rx) = mpsc::unbounded_channel();
    let (release_tx, release_rx) = watch::channel(false);
    let mut tasks = Vec::new();

    for _ in 0..64 {
        let binding = binding.clone();
        let result_tx = result_tx.clone();
        let mut release_rx = release_rx.clone();
        tasks.push(tokio::spawn(async move {
            let permit = binding.try_acquire();
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

    let mut acquired = 0;
    for _ in 0..64 {
        acquired += usize::from(result_rx.recv().await.expect("task result"));
    }
    assert_eq!(acquired, 4);
    assert_eq!(binding.capacity().in_flight(), 4);

    release_tx
        .send(true)
        .expect("release receivers remain open");
    for task in tasks {
        task.await.expect("acquire task");
    }
    assert_eq!(binding.capacity().in_flight(), 0);
    assert_eq!(runtime.scheduler_epoch(), 4);
    scheduler_epoch
        .changed()
        .await
        .expect("runtime owns the scheduler epoch sender");
    assert_eq!(*scheduler_epoch.borrow_and_update(), 4);
}

#[test]
fn lowering_capacity_preserves_in_flight_and_blocks_new_acquires() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let initial = runtime.reconcile_configuration(&fixture.configuration(3, 1, 1));
    let binding = initial.as_slice()[0].clone();
    let first = binding.try_acquire().expect("first permit");
    let second = binding.try_acquire().expect("second permit");
    let third = binding.try_acquire().expect("third permit");

    let lowered = runtime.reconcile_configuration(&fixture.configuration(1, 1, 1));
    let lowered = &lowered.as_slice()[0];
    assert_eq!(lowered.capacity().in_flight(), 3);
    assert_eq!(lowered.capacity().max_concurrency(), 1);
    assert!(lowered.try_acquire().is_none());

    drop(first);
    assert!(lowered.try_acquire().is_none());
    drop(second);
    assert!(lowered.try_acquire().is_none());
    drop(third);
    assert!(lowered.try_acquire().is_some());
}

#[test]
fn raising_capacity_allows_new_acquires_immediately() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let initial = runtime.reconcile_configuration(&fixture.configuration(1, 1, 1));
    let binding = initial.as_slice()[0].clone();
    let first = binding.try_acquire().expect("initial permit");
    assert!(binding.try_acquire().is_none());

    let raised = runtime.reconcile_configuration(&fixture.configuration(3, 1, 1));
    let raised = &raised.as_slice()[0];
    let second = raised.try_acquire().expect("second permit after raise");
    let third = raised.try_acquire().expect("third permit after raise");
    assert!(raised.try_acquire().is_none());

    drop((first, second, third));
}

#[test]
fn generation_changes_are_pinned_without_resetting_capacity() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let initial = runtime.reconcile_configuration(&fixture.configuration(2, 1, 1));
    let old_binding = initial.as_slice()[0].clone();
    let old_permit = old_binding.try_acquire().expect("old generation permit");

    let rotated = runtime.reconcile_configuration(&fixture.configuration(2, 2, 2));
    let new_binding = rotated.as_slice()[0].clone();
    assert_eq!(old_permit.generation().credential_generation(), 1);
    assert_eq!(new_binding.generation().credential_generation(), 2);
    assert_eq!(new_binding.generation().secret_version(), 2);
    assert!(!Arc::ptr_eq(
        old_permit.generation(),
        new_binding.generation()
    ));
    assert_eq!(new_binding.capacity().in_flight(), 1);

    drop(old_permit);
    let new_permit = new_binding.try_acquire().expect("new generation permit");
    assert_eq!(new_permit.generation().credential_generation(), 2);
}

#[test]
fn removed_credentials_retire_without_invalidating_old_bindings() {
    let runtime = RuntimeRegistry::new();
    let fixture = CredentialFixture::new();
    let bindings = runtime.reconcile_configuration(&fixture.configuration(1, 1, 1));
    let old_binding = bindings.as_slice()[0].clone();

    runtime.reconcile_configuration(&ProviderCredentialConfiguration::initial());

    assert_eq!(runtime.active_credential_count(), 0);
    assert!(old_binding.is_retired());
    assert!(old_binding.try_acquire().is_some());
}

#[test]
fn selector_uses_exact_load_ratios_and_rotating_ties() {
    let first_runtime = RuntimeRegistry::new();
    let first_fixture = CredentialFixture::new();
    let first = first_runtime
        .reconcile_configuration(&first_fixture.configuration(10, 1, 1))
        .as_slice()[0]
        .clone();
    let held = (0..5)
        .map(|_| first.try_acquire().expect("first credential capacity"))
        .collect::<Vec<_>>();

    let second_runtime = RuntimeRegistry::new();
    let second_fixture = CredentialFixture::new();
    let second = second_runtime
        .reconcile_configuration(&second_fixture.configuration(2, 1, 1))
        .as_slice()[0]
        .clone();
    let selected = select_and_try_acquire(&[first.clone(), second.clone()], 0);
    let SelectAndAcquireResult::Acquired(selected) = selected else {
        panic!("an available credential must be selected");
    };
    assert_eq!(selected.credential_id(), second.credential_id());
    drop(selected);
    drop(held);

    let tie = select_and_try_acquire(&[first, second.clone()], 1);
    let SelectAndAcquireResult::Acquired(tie) = tie else {
        panic!("an equal-load credential must be selected");
    };
    assert_eq!(tie.credential_id(), second.credential_id());
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
                false,
                false,
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
        max_concurrency: u32,
        credential_generation: u64,
        secret_version: u64,
    ) -> ProviderCredentialConfiguration {
        let draft = ProviderCredentialDraft::new(
            "Primary",
            CredentialKind::ApiKey,
            ProxyProfileId::DIRECT,
            MaxConcurrency::new(max_concurrency).expect("max concurrency"),
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
            1,
            secret_version,
            credential_generation,
            credential_generation,
        )
        .expect("credential");
        ProviderCredentialConfiguration::new(vec![credential], &self.endpoints, &self.proxies)
            .expect("credential configuration")
    }
}
