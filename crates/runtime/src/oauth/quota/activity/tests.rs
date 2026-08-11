use super::*;

use any2api_provider::api::{OAuthQuotaRateLimit, OAuthQuotaWindow, OAuthQuotaWindowKind};

#[test]
fn guard_prices_the_final_cumulative_token_usage_once() {
    let activity = OAuthQuotaActivity::new();
    let id = OAuthAccountId::new();
    assert!(
        activity
            .observe_snapshot(id, &quota_usage(10.0), 100)
            .is_empty()
    );
    let mut guard = activity.guard(
        id,
        Some(OAuthQuotaCostRate::new("test_rate_card", 5.0, 0.5, 30.0)),
    );
    guard.observe_token_usage(TokenUsage::new(Some(2_000), Some(10), Some(500)));
    guard.observe_token_usage(TokenUsage::new(None, Some(100), None));
    drop(guard);

    let estimates = activity.observe_snapshot(id, &quota_usage(11.0), 400);

    assert_eq!(estimates.len(), 1);
    assert!((estimates[0].sample_cost_usd - 0.010_75).abs() < f64::EPSILON);
    assert!((estimates[0].estimated_capacity_usd - 1.075).abs() < f64::EPSILON);
}

#[tokio::test(start_paused = true)]
async fn idle_accounts_have_no_due_work_and_bursts_coalesce() {
    let activity = OAuthQuotaActivity::new();
    assert_eq!(activity.next_due(), None);
    let id = OAuthAccountId::new();
    activity.record(id, Instant::now());
    activity.record(id, Instant::now() + Duration::from_secs(2));

    assert!(
        activity
            .take_due(Instant::now() + Duration::from_secs(4), 6)
            .is_empty()
    );
    assert_eq!(
        activity.take_due(Instant::now() + ACTIVITY_DEBOUNCE, 6),
        vec![id]
    );
    assert!(
        activity
            .take_due(Instant::now() + ACTIVITY_DEBOUNCE, 6)
            .is_empty()
    );
}

#[tokio::test(start_paused = true)]
async fn subsequent_activity_obeys_minimum_interval() {
    let activity = OAuthQuotaActivity::new();
    let id = OAuthAccountId::new();
    let start = Instant::now();
    activity.record(id, start);
    assert_eq!(activity.take_due(start + ACTIVITY_DEBOUNCE, 1), vec![id]);
    activity.complete(id, start + ACTIVITY_DEBOUNCE);
    activity.record(id, start + Duration::from_secs(6));

    assert!(
        activity
            .take_due(
                start + ACTIVITY_DEBOUNCE + MIN_REFRESH_INTERVAL - Duration::from_millis(1),
                1,
            )
            .is_empty()
    );
    assert_eq!(
        activity.take_due(start + ACTIVITY_DEBOUNCE + MIN_REFRESH_INTERVAL, 1),
        vec![id]
    );
}

#[tokio::test(start_paused = true)]
async fn activity_during_refresh_schedules_one_follow_up_but_failure_without_activity_does_not() {
    let activity = OAuthQuotaActivity::new();
    let id = OAuthAccountId::new();
    let start = Instant::now();
    activity.record(id, start);
    assert_eq!(activity.take_due(start + ACTIVITY_DEBOUNCE, 1), vec![id]);
    activity.record(id, start + Duration::from_secs(10));
    activity.record(id, start + Duration::from_secs(11));
    activity.complete(id, start + Duration::from_secs(12));
    assert_eq!(
        activity.next_due(),
        Some(start + ACTIVITY_DEBOUNCE + MIN_REFRESH_INTERVAL)
    );

    assert_eq!(
        activity.take_due(start + ACTIVITY_DEBOUNCE + MIN_REFRESH_INTERVAL, 1),
        vec![id]
    );
    activity.complete(id, start + ACTIVITY_DEBOUNCE + MIN_REFRESH_INTERVAL);
    assert_eq!(activity.next_due(), None);
}

#[tokio::test(start_paused = true)]
async fn due_selection_enforces_the_global_concurrency_bound() {
    let activity = OAuthQuotaActivity::new();
    let start = Instant::now();
    let ids = (0..8)
        .map(|_| {
            let id = OAuthAccountId::new();
            activity.record(id, start);
            id
        })
        .collect::<Vec<_>>();

    let first = activity.take_due(start + ACTIVITY_DEBOUNCE, MAX_CONCURRENT_REFRESHES);
    assert_eq!(first.len(), MAX_CONCURRENT_REFRESHES);
    assert!(
        activity
            .take_due(
                start + ACTIVITY_DEBOUNCE,
                MAX_CONCURRENT_REFRESHES - first.len()
            )
            .is_empty()
    );

    activity.complete(first[0], start + ACTIVITY_DEBOUNCE);
    let next = activity.take_due(start + ACTIVITY_DEBOUNCE, 1);
    assert_eq!(next.len(), 1);
    assert!(ids.contains(&next[0]));
    assert!(!first.contains(&next[0]));
}

fn quota_usage(used_percent: f64) -> OAuthQuotaUsage {
    OAuthQuotaUsage {
        rate_limit: Some(OAuthQuotaRateLimit {
            allowed: None,
            limit_reached: None,
            windows: vec![OAuthQuotaWindow {
                id: "primary".to_owned(),
                kind: OAuthQuotaWindowKind::Time,
                used_percent,
                limit_window_seconds: Some(18_000),
                reset_after_seconds: Some(300),
                reset_at: Some(1_900_000_000),
            }],
        }),
        credits: None,
        access: None,
        reset_credits: None,
        billing: None,
        token_balance: None,
        subscription_tier: None,
        account_status: None,
    }
}
