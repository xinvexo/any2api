use std::{collections::BTreeMap, sync::Arc, time::Duration};

use any2api_domain::{
    RetrySafety, RouteTargetId, UpstreamErrorClassification, UpstreamErrorKind,
    UpstreamFailureAttribution,
};

use super::super::{
    GenerationSelection, filter_recorder::RequestFilterRecorder, fixed as fixed_selection,
    try_select_fixed_candidate_for_test, try_select_generation_candidate_for_test,
};
use super::selection::{candidate, default_reliability_policy, open_endpoint};
use crate::{
    health::EndpointHealthRuntime,
    routing::{QueueCoordinator, SchedulerEpoch},
};

#[test]
fn fixed_selection_records_the_successful_selection() {
    let epoch = SchedulerEpoch::new();
    let candidate = candidate("fixed", 5, Arc::clone(&epoch), 0);
    let selected =
        try_select_fixed_candidate_for_test(default_reliability_policy(), &candidate, || {})
            .expect("fixed selection")
            .expect("fixed RPM reservation");

    assert_eq!(candidate.binding.balancing_counters().selected(), 1);
    assert_eq!(candidate.binding.in_flight(), 1);
    assert_eq!(candidate.binding.rate_snapshot().requests_in_window(), 1);
    drop(selected);
    assert_eq!(candidate.binding.in_flight(), 0);
    assert_eq!(candidate.binding.rate_snapshot().requests_in_window(), 1);
}

#[test]
fn fixed_rechecks_record_one_rate_filter_per_request() {
    let epoch = SchedulerEpoch::new();
    let candidate = candidate("fixed-filter", 7, Arc::clone(&epoch), 0);
    drop(candidate.binding.try_reserve().expect("exhaust RPM"));
    let mut filters = RequestFilterRecorder::default();

    for _ in 0..5 {
        assert!(
            fixed_selection::try_selected_with_recorder_for_test(
                default_reliability_policy(),
                &candidate,
                &mut filters,
            )
            .expect("fixed selection")
            .is_none()
        );
    }

    assert_eq!(
        candidate.binding.balancing_counters().filtered_rate_limit(),
        1
    );
}

#[tokio::test(start_paused = true)]
async fn fixed_selection_rolls_back_rpm_when_the_half_open_probe_is_raced() {
    let epoch = SchedulerEpoch::new();
    let policy = default_reliability_policy();
    let endpoint = EndpointHealthRuntime::new(Arc::clone(&epoch));
    open_endpoint(&endpoint, &policy);
    tokio::time::advance(policy.endpoint_open_duration).await;

    let mut candidate = candidate("fixed-raced", 6, Arc::clone(&epoch), 0);
    candidate.endpoint_health = Some(endpoint);
    let candidate_for_probe = candidate.clone();
    let mut occupied_probe = None;

    let selected = try_select_fixed_candidate_for_test(policy, &candidate, || {
        occupied_probe = Some(
            candidate_for_probe
                .acquire_health(policy)
                .expect("half-open probe"),
        );
    })
    .expect("fixed selection result");

    assert!(selected.is_none());
    assert_eq!(candidate.binding.in_flight(), 0);
    assert_eq!(candidate.binding.rate_snapshot().requests_in_window(), 0);
    assert_eq!(
        candidate
            .binding
            .balancing_counters()
            .filtered_endpoint_health(),
        1
    );
    drop(occupied_probe);
}

#[tokio::test(start_paused = true)]
async fn fixed_health_wait_does_not_block_another_model_on_the_same_credential() {
    let epoch = SchedulerEpoch::new();
    let policy = default_reliability_policy();
    let fixed_candidate = candidate("fixed-health", 7, Arc::clone(&epoch), 0);
    fixed_candidate.binding.generation().health().record(
        &fixed_candidate.upstream_model,
        UpstreamErrorClassification::new(
            UpstreamErrorKind::RateLimited,
            RetrySafety::RejectedBeforeExecution,
            None,
        )
        .with_attribution(UpstreamFailureAttribution::CredentialModel),
        &policy,
    );
    let mut healthy_model = fixed_candidate.clone();
    healthy_model.target_id = RouteTargetId::new();
    healthy_model.upstream_model = "healthy-model".to_owned();
    let queue = QueueCoordinator::new(Arc::clone(&epoch));
    let queued_candidate = fixed_candidate.clone();
    let queued = Arc::clone(&queue);
    let task = tokio::spawn(async move {
        fixed_selection::select_with_queue_for_test(
            &queued,
            1,
            policy,
            &queued_candidate,
            Duration::from_secs(30),
        )
        .await
    });

    for _ in 0..10_000 {
        if queue.waiting_count() == 1 {
            break;
        }
        tokio::task::yield_now().await;
    }
    assert_eq!(queue.waiting_count(), 1);
    assert_eq!(fixed_candidate.binding.fixed_waiter_count(), 0);

    let tiers = BTreeMap::from([(0, vec![healthy_model.clone()])]);
    let selected = match try_select_generation_candidate_for_test(false, &tiers, |_| Some(0))
        .expect("healthy model selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        GenerationSelection::RateLimited(_) => panic!("health wait must not reserve RPM"),
        GenerationSelection::TemporarilyUnavailable(_) => panic!("other model is healthy"),
        GenerationSelection::RetryDeferred(_) => panic!("no retry deferral exists"),
        GenerationSelection::NoCandidates => panic!("other model exists"),
    };
    assert_eq!(
        selected.candidate.upstream_model,
        healthy_model.upstream_model
    );
    drop(selected);

    task.abort();
    assert!(task.await.is_err());
    assert_eq!(queue.waiting_count(), 0);
}
