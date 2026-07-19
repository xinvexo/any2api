use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    time::Duration,
};

use any2api_domain::{
    CredentialId, CredentialKind, CredentialSecretFingerprint, MaxConcurrency, ProviderCredential,
    ProviderCredentialDraft, ProviderEndpointId, ProxyProfileId, PublicErrorCode, RouteTargetId,
};

use super::{
    GenerationSelection, RequestPermit, RouteCandidate, SelectedCandidate,
    select_auxiliary_candidate_with, select_generation_candidate,
    try_select_generation_candidate_with, wait_for_generation_candidate,
};
use crate::{
    auxiliary_scheduler::{
        AuxiliaryConcurrencyLimits, AuxiliaryScheduler, AuxiliarySelectAndAcquireResult,
    },
    credential_auth::CredentialAuthMaterial,
    credential_runtime::CredentialRuntimeHandle,
    queue::{QueueCoordinator, QueuePolicy, SaturationAction},
    scheduler_epoch::SchedulerEpoch,
};

#[test]
fn auxiliary_saturation_does_not_fall_through_to_a_later_tier() {
    let epoch = SchedulerEpoch::new();
    let scheduler = AuxiliaryScheduler::new(
        AuxiliaryConcurrencyLimits::new(1, 1).expect("limits"),
        Arc::clone(&epoch),
    );
    let primary = candidate("primary", 1, Arc::clone(&epoch), 0);
    let fallback = candidate("fallback", 2, Arc::clone(&epoch), 1);
    let primary_slot =
        match scheduler.select_index_and_try_acquire(std::slice::from_ref(&primary.binding), 0) {
            AuxiliarySelectAndAcquireResult::Acquired { permit, .. } => permit,
            AuxiliarySelectAndAcquireResult::AtCapacity => panic!("primary slot available"),
            AuxiliarySelectAndAcquireResult::NoCandidates => panic!("primary candidate exists"),
        };
    let tiers = BTreeMap::from([(0, vec![primary]), (1, vec![fallback.clone()])]);

    let error = match select_auxiliary_candidate_with(&scheduler, &tiers, |_| Some(0)) {
        Ok(_) => panic!("primary saturation must fail immediately"),
        Err(error) => error,
    };

    assert_eq!(error.code, PublicErrorCode::LocalConcurrencyLimit);
    assert_eq!(fallback.binding.auxiliary_in_flight(), 0);
    drop(primary_slot);
}

#[test]
fn generation_fallback_only_skips_a_saturated_tier_when_enabled() {
    let epoch = SchedulerEpoch::new();
    let primary = candidate("primary", 1, Arc::clone(&epoch), 0);
    let fallback = candidate("fallback", 2, Arc::clone(&epoch), 1);
    let blocker = primary.binding.try_acquire().expect("primary blocker");
    let tiers = BTreeMap::from([(0, vec![primary]), (1, vec![fallback.clone()])]);

    assert!(matches!(
        try_select_generation_candidate_with(false, &tiers, |_| Some(0)),
        Ok(GenerationSelection::AtCapacity)
    ));
    let selected = match try_select_generation_candidate_with(true, &tiers, |_| Some(0))
        .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        GenerationSelection::AtCapacity => panic!("fallback capacity is available"),
        GenerationSelection::NoCandidates => panic!("fallback candidate exists"),
    };
    assert_eq!(selected.candidate.credential_id, fallback.credential_id);
    drop(selected);
    drop(blocker);
}

#[test]
fn generation_selection_reports_no_candidates_for_empty_tiers() {
    let tiers = BTreeMap::new();

    assert!(matches!(
        try_select_generation_candidate_with(false, &tiers, |_| Some(0)),
        Ok(GenerationSelection::NoCandidates)
    ));
}

#[tokio::test]
async fn reject_policy_does_not_enter_the_queue() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(epoch);
    let policy = policy(SaturationAction::Reject, Duration::from_secs(1), 1);

    let error = match select_generation_candidate(&coordinator, policy, || {
        Ok(GenerationSelection::AtCapacity)
    })
    .await
    {
        Ok(_) => panic!("reject policy must fail immediately"),
        Err(error) => error,
    };

    assert_eq!(error.code, PublicErrorCode::LocalConcurrencyLimit);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test]
async fn queue_limit_rejects_an_additional_waiter() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(epoch);
    let occupied = coordinator.try_ticket(1).expect("occupied ticket");
    let policy = policy(SaturationAction::Wait, Duration::from_secs(1), 1);

    let error = match select_generation_candidate(&coordinator, policy, || {
        Ok(GenerationSelection::AtCapacity)
    })
    .await
    {
        Ok(_) => panic!("bounded queue must reject another waiter"),
        Err(error) => error,
    };

    assert_eq!(error.code, PublicErrorCode::LocalConcurrencyLimit);
    assert_eq!(error.message, "request queue is full");
    assert_eq!(coordinator.waiting_count(), 1);
    drop(occupied);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test]
async fn generation_wait_reselects_after_a_permit_is_released() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("queued", 1, Arc::clone(&epoch), 0);
    let blocker = candidate.binding.try_acquire().expect("blocker permit");
    let queued_candidate = candidate.clone();
    let policy = policy(SaturationAction::Wait, Duration::from_secs(1), 1);
    let coordinator_for_task = Arc::clone(&coordinator);
    let task = tokio::spawn(async move {
        wait_for_generation_candidate(&coordinator_for_task, policy, || {
            try_acquire_candidate(&queued_candidate)
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    drop(blocker);
    let selected = task.await.expect("queue task").expect("selected candidate");
    assert_eq!(selected.candidate.credential_id, candidate.credential_id);
    drop(selected);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test(start_paused = true)]
async fn generation_wait_times_out_and_releases_its_ticket() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("timeout", 1, Arc::clone(&epoch), 0);
    let blocker = candidate.binding.try_acquire().expect("blocker permit");
    let policy = policy(SaturationAction::Wait, Duration::from_secs(1), 1);
    let queued_candidate = candidate.clone();
    let coordinator_for_task = Arc::clone(&coordinator);
    let task = tokio::spawn(async move {
        wait_for_generation_candidate(&coordinator_for_task, policy, || {
            try_acquire_candidate(&queued_candidate)
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(1)).await;
    let error = match task.await.expect("queue task") {
        Ok(_) => panic!("queue must time out"),
        Err(error) => error,
    };

    assert_eq!(error.code, PublicErrorCode::LocalConcurrencyLimit);
    assert_eq!(coordinator.waiting_count(), 0);
    drop(blocker);
}

#[tokio::test(start_paused = true)]
async fn timeout_boundary_performs_one_final_selection() {
    let coordinator = QueueCoordinator::new(SchedulerEpoch::new());
    let candidate = candidate("deadline", 1, SchedulerEpoch::new(), 0);
    let policy = policy(SaturationAction::Wait, Duration::from_secs(1), 1);
    let attempts = Arc::new(AtomicUsize::new(0));
    let attempts_for_task = Arc::clone(&attempts);
    let coordinator_for_task = Arc::clone(&coordinator);
    let candidate_for_task = candidate.clone();
    let task = tokio::spawn(async move {
        select_generation_candidate(&coordinator_for_task, policy, || {
            let attempt = attempts_for_task.fetch_add(1, Ordering::AcqRel) + 1;
            if attempt < 3 {
                Ok(GenerationSelection::AtCapacity)
            } else {
                try_acquire_candidate(&candidate_for_task)
            }
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    for _ in 0..100 {
        if attempts.load(Ordering::Acquire) >= 2 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(attempts.load(Ordering::Acquire), 2);
    tokio::task::yield_now().await;
    tokio::time::advance(Duration::from_secs(1)).await;
    let selected = task
        .await
        .expect("queue task")
        .expect("final selection at the timeout boundary");

    assert_eq!(attempts.load(Ordering::Acquire), 3);
    assert_eq!(selected.candidate.credential_id, candidate.credential_id);
    drop(selected);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test]
async fn epoch_advance_between_recheck_and_wait_is_not_lost() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("wake", 1, Arc::clone(&epoch), 0);
    let policy = policy(SaturationAction::Wait, Duration::from_secs(1), 1);
    let mut attempts = 0_u8;

    let selected = wait_for_generation_candidate(&coordinator, policy, || {
        attempts += 1;
        if attempts == 1 {
            epoch.advance();
            Ok(GenerationSelection::AtCapacity)
        } else {
            try_acquire_candidate(&candidate)
        }
    })
    .await
    .expect("epoch notification must wake the waiter");

    assert_eq!(attempts, 2);
    drop(selected);
    assert_eq!(coordinator.waiting_count(), 0);
}

#[tokio::test]
async fn cancellation_drops_the_queue_ticket() {
    let epoch = SchedulerEpoch::new();
    let coordinator = QueueCoordinator::new(Arc::clone(&epoch));
    let candidate = candidate("cancel", 1, Arc::clone(&epoch), 0);
    let blocker = candidate.binding.try_acquire().expect("blocker permit");
    let policy = policy(SaturationAction::Wait, Duration::from_secs(30), 1);
    let queued_candidate = candidate.clone();
    let coordinator_for_task = Arc::clone(&coordinator);
    let task = tokio::spawn(async move {
        wait_for_generation_candidate(&coordinator_for_task, policy, || {
            try_acquire_candidate(&queued_candidate)
        })
        .await
    });

    wait_until_waiting(&coordinator, 1).await;
    task.abort();
    assert!(task.await.is_err());
    assert_eq!(coordinator.waiting_count(), 0);
    assert_eq!(candidate.binding.capacity().in_flight(), 1);
    drop(blocker);
}

async fn wait_until_waiting(coordinator: &QueueCoordinator, expected: u32) {
    for _ in 0..10_000 {
        if coordinator.waiting_count() == expected {
            return;
        }
        tokio::task::yield_now().await;
    }
    panic!("queue task did not start");
}

fn try_acquire_candidate(
    candidate: &RouteCandidate,
) -> Result<GenerationSelection, any2api_domain::PublicError> {
    let Some(permit) = candidate.binding.try_acquire() else {
        return Ok(GenerationSelection::AtCapacity);
    };
    Ok(GenerationSelection::Acquired(SelectedCandidate {
        candidate: candidate.clone(),
        permit: RequestPermit::Generation(permit),
    }))
}

fn policy(action: SaturationAction, timeout: Duration, max_waiting: u32) -> QueuePolicy {
    QueuePolicy::new(action, timeout, max_waiting, false).expect("queue policy")
}

fn candidate(
    label: &str,
    fingerprint_byte: u8,
    scheduler_epoch: Arc<SchedulerEpoch>,
    tier: u16,
) -> RouteCandidate {
    let credential = ProviderCredential::create(
        CredentialId::new(),
        ProviderEndpointId::new(),
        ProviderCredentialDraft::new(
            label,
            CredentialKind::ApiKey,
            ProxyProfileId::DIRECT,
            MaxConcurrency::new(1).expect("max concurrency"),
            true,
        )
        .expect("credential draft"),
        CredentialSecretFingerprint::new([fingerprint_byte; 32], None).expect("fingerprint"),
    );
    let binding = CredentialRuntimeHandle::new(
        &credential,
        CredentialAuthMaterial::for_test(&credential, format!("sk-{label}-test")),
        scheduler_epoch,
    )
    .current_binding();
    RouteCandidate {
        target_id: RouteTargetId::new(),
        endpoint_id: credential.provider_endpoint_id(),
        credential_id: credential.id(),
        upstream_model: format!("upstream-{tier}"),
        binding,
    }
}
