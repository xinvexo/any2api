use std::{
    collections::HashMap,
    sync::{Arc, Barrier, mpsc},
    thread,
};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, MaxConcurrency, ProviderCredential,
    ProviderCredentialDraft, ProviderEndpointId, ProxyProfileId,
};

use crate::{
    auxiliary_scheduler::{
        AuxiliaryConcurrencyLimits, AuxiliaryConcurrencyLimitsError, AuxiliaryPermit,
        AuxiliaryScheduler, AuxiliarySelectAndAcquireResult,
    },
    credential_auth::CredentialAuthMaterial,
    credential_runtime::{CredentialRuntimeBinding, CredentialRuntimeHandle},
    scheduler_epoch::SchedulerEpoch,
};

#[test]
fn auxiliary_limits_reject_zero_values() {
    assert_eq!(
        AuxiliaryConcurrencyLimits::new(0, 1),
        Err(AuxiliaryConcurrencyLimitsError::ZeroGlobal)
    );
    assert_eq!(
        AuxiliaryConcurrencyLimits::new(1, 0),
        Err(AuxiliaryConcurrencyLimitsError::ZeroPerCredential)
    );
}

#[test]
fn auxiliary_and_generation_capacity_are_independent() {
    let epoch = SchedulerEpoch::new();
    let scheduler = AuxiliaryScheduler::new(limits(1, 1), Arc::clone(&epoch));
    let binding = binding("primary", 1, 1, Arc::clone(&epoch));

    let generation = binding.try_acquire().expect("generation permit");
    assert_eq!(binding.capacity().in_flight(), 1);
    let auxiliary = acquire(&scheduler, std::slice::from_ref(&binding), 0);
    assert_eq!(binding.capacity().in_flight(), 1);
    assert_eq!(binding.auxiliary_in_flight(), 1);
    assert_eq!(auxiliary.credential_id(), binding.credential_id());
    assert_eq!(
        auxiliary.generation().credential_generation(),
        binding.generation().credential_generation()
    );

    drop(generation);
    let next_generation = binding
        .try_acquire()
        .expect("generation capacity is independent from auxiliary capacity");
    assert_eq!(binding.auxiliary_in_flight(), 1);
    drop(next_generation);
    drop(auxiliary);

    assert_eq!(binding.capacity().in_flight(), 0);
    assert_eq!(binding.auxiliary_in_flight(), 0);
    assert_eq!(scheduler.global_in_flight(), 0);
}

#[test]
fn auxiliary_limit_updates_preserve_existing_usage() {
    let epoch = SchedulerEpoch::new();
    let scheduler = AuxiliaryScheduler::new(limits(3, 2), Arc::clone(&epoch));
    let binding = binding("primary", 2, 1, Arc::clone(&epoch));
    let first = acquire(&scheduler, std::slice::from_ref(&binding), 0);
    let second = acquire(&scheduler, std::slice::from_ref(&binding), 0);

    scheduler.update_limits(limits(1, 1));
    assert_eq!(scheduler.limits(), limits(1, 1));
    assert!(matches!(
        scheduler.select_index_and_try_acquire(std::slice::from_ref(&binding), 0),
        AuxiliarySelectAndAcquireResult::AtCapacity
    ));

    drop(first);
    assert!(matches!(
        scheduler.select_index_and_try_acquire(std::slice::from_ref(&binding), 0),
        AuxiliarySelectAndAcquireResult::AtCapacity
    ));
    drop(second);

    let after_drain = acquire(&scheduler, std::slice::from_ref(&binding), 0);
    drop(after_drain);
    assert_eq!(binding.auxiliary_in_flight(), 0);
    assert_eq!(scheduler.global_in_flight(), 0);
}

#[test]
fn concurrent_auxiliary_acquires_never_exceed_either_limit() {
    const THREADS: usize = 12;
    let epoch = SchedulerEpoch::new();
    let scheduler = AuxiliaryScheduler::new(limits(5, 2), Arc::clone(&epoch));
    let bindings = Arc::new(vec![
        binding("a", 4, 1, Arc::clone(&epoch)),
        binding("b", 4, 2, Arc::clone(&epoch)),
        binding("c", 4, 3, Arc::clone(&epoch)),
    ]);
    let start = Arc::new(Barrier::new(THREADS + 1));
    let release = Arc::new(Barrier::new(THREADS + 1));
    let (sender, receiver) = mpsc::channel();
    let mut workers = Vec::new();

    for tie_breaker in 0..THREADS {
        let scheduler = Arc::clone(&scheduler);
        let bindings = Arc::clone(&bindings);
        let start = Arc::clone(&start);
        let release = Arc::clone(&release);
        let sender = sender.clone();
        workers.push(thread::spawn(move || {
            start.wait();
            let permit = match scheduler
                .select_index_and_try_acquire(bindings.as_slice(), tie_breaker as u64)
            {
                AuxiliarySelectAndAcquireResult::Acquired { permit, .. } => Some(permit),
                AuxiliarySelectAndAcquireResult::AtCapacity => None,
                AuxiliarySelectAndAcquireResult::NoCandidates => {
                    panic!("test supplied candidates")
                }
            };
            sender
                .send(permit.as_ref().map(AuxiliaryPermit::credential_id))
                .expect("send acquire result");
            release.wait();
            drop(permit);
        }));
    }
    drop(sender);

    start.wait();
    let selected = receiver.iter().take(THREADS).collect::<Vec<_>>();
    let acquired = selected.iter().flatten().count();
    assert_eq!(acquired, 5);
    let mut by_credential = HashMap::new();
    for credential_id in selected.into_iter().flatten() {
        *by_credential.entry(credential_id).or_insert(0_usize) += 1;
    }
    assert!(by_credential.values().all(|count| *count <= 2));
    assert_eq!(scheduler.global_in_flight(), 5);

    release.wait();
    for worker in workers {
        worker.join().expect("auxiliary acquire worker");
    }
    assert_eq!(scheduler.global_in_flight(), 0);
    assert!(
        bindings
            .iter()
            .all(|binding| binding.auxiliary_in_flight() == 0)
    );
    assert_eq!(epoch.current(), 5);
}

fn acquire(
    scheduler: &Arc<AuxiliaryScheduler>,
    bindings: &[CredentialRuntimeBinding],
    tie_breaker: u64,
) -> AuxiliaryPermit {
    match scheduler.select_index_and_try_acquire(bindings, tie_breaker) {
        AuxiliarySelectAndAcquireResult::Acquired { permit, .. } => permit,
        AuxiliarySelectAndAcquireResult::AtCapacity => panic!("auxiliary capacity available"),
        AuxiliarySelectAndAcquireResult::NoCandidates => panic!("test supplied candidates"),
    }
}

fn limits(global: u32, per_credential: u32) -> AuxiliaryConcurrencyLimits {
    AuxiliaryConcurrencyLimits::new(global, per_credential).expect("test limits")
}

fn binding(
    label: &str,
    max_concurrency: u32,
    fingerprint_byte: u8,
    scheduler_epoch: Arc<SchedulerEpoch>,
) -> CredentialRuntimeBinding {
    let credential = ProviderCredential::create(
        CredentialId::new(),
        ProviderEndpointId::new(),
        ProviderCredentialDraft::new(
            label,
            CredentialKind::ApiKey,
            ProxyProfileId::DIRECT,
            MaxConcurrency::new(max_concurrency).expect("max concurrency"),
            true,
        )
        .expect("credential draft"),
        CredentialSecretFingerprint::new([fingerprint_byte; 32], None).expect("fingerprint"),
    );
    CredentialRuntimeHandle::new(
        &credential,
        CredentialAuthMaterial::for_test(&credential, format!("sk-{label}-test")),
        scheduler_epoch,
    )
    .current_binding()
}
