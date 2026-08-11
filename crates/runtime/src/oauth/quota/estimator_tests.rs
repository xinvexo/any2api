use any2api_provider::{
    CodexDriver,
    api::{OAuthQuotaUsage, OAuthQuotaWindow, OAuthQuotaWindowKind},
};
use any2api_storage::api::{OAuthQuotaRequestLogModelUsage, OAuthQuotaRequestLogUsage};

use super::{
    estimator::{estimate_from_logs, prior_estimate, window_bounds},
    types::{OAuthQuotaSnapshot, OAuthQuotaUsdEstimate},
};

#[test]
fn complete_window_logs_infer_capacity_from_current_percent() {
    let estimate = estimate_from_logs(
        &CodexDriver::new(),
        &window(1.0),
        &log_usage("gpt-5.5", 2_000, 0, 0),
        1_000_000,
        2_000_000,
    )
    .expect("estimate");

    assert!((estimate.estimated_used_usd - 0.01).abs() < f64::EPSILON);
    assert!((estimate.estimated_capacity_usd - 1.0).abs() < f64::EPSILON);
    assert!((estimate.estimated_remaining_usd - 0.99).abs() < f64::EPSILON);
    assert_eq!(estimate.sample_used_percent, 1.0);
}

#[test]
fn cached_tokens_are_priced_and_incomplete_or_unknown_logs_are_rejected() {
    let driver = CodexDriver::new();
    let estimate = estimate_from_logs(
        &driver,
        &window(10.0),
        &log_usage("gpt-5.5", 2_000, 100, 500),
        1_000_000,
        2_000_000,
    )
    .expect("estimate");
    assert!((estimate.sample_cost_usd - 0.010_75).abs() < f64::EPSILON);

    let mut incomplete = log_usage("gpt-5.5", 2_000, 100, 500);
    incomplete.records_complete = false;
    assert!(
        estimate_from_logs(&driver, &window(10.0), &incomplete, 1_000_000, 2_000_000).is_none()
    );
    assert!(
        estimate_from_logs(
            &driver,
            &window(10.0),
            &log_usage("unknown", 2_000, 100, 500),
            1_000_000,
            2_000_000
        )
        .is_none()
    );
}

#[test]
fn local_reset_boundary_excludes_the_old_part_of_the_upstream_window() {
    let mut window = window(1.0);
    window.limit_window_seconds = Some(18_000);
    window.reset_at = Some(20_000);

    assert_eq!(
        window_bounds(&window, 3_000_000, Some(2_500_000)),
        Some((2_500_000, 3_000_000))
    );
    assert_eq!(window_bounds(&window, 3_000_000, Some(3_000_000)), None);
}

#[test]
fn saturated_credits_only_reuse_an_estimate_from_after_the_reset_boundary() {
    let window = window(100.0);
    let mut snapshot = OAuthQuotaSnapshot {
        usage: empty_usage(),
        usd_estimates: vec![OAuthQuotaUsdEstimate {
            window_id: window.id.clone(),
            window_kind: window.kind,
            limit_window_seconds: window.limit_window_seconds,
            window_reset_at: window.reset_at,
            estimated_capacity_usd: 10.0,
            estimated_used_usd: 10.0,
            estimated_remaining_usd: 0.0,
            sample_cost_usd: 10.0,
            sample_used_percent: 100.0,
            sample_started_at: 2_000,
            sample_ended_at: 2_500,
            pricing_basis: "test".into(),
        }],
        fetched_at: 2_500,
    };
    assert!(prior_estimate(&snapshot, &window, 2_000_456).is_some());
    assert!(prior_estimate(&snapshot, &window, 2_001_000).is_none());
    snapshot.usd_estimates[0].window_reset_at = Some(99);
    assert!(prior_estimate(&snapshot, &window, 2_000_000).is_none());
}

fn window(used_percent: f64) -> OAuthQuotaWindow {
    OAuthQuotaWindow {
        id: "primary".into(),
        kind: OAuthQuotaWindowKind::Time,
        used_percent,
        limit_window_seconds: Some(1_000),
        reset_after_seconds: None,
        reset_at: Some(2_000),
    }
}

fn empty_usage() -> OAuthQuotaUsage {
    OAuthQuotaUsage {
        rate_limit: None,
        credits: None,
        access: None,
        reset_credits: None,
        billing: None,
        token_balance: None,
        subscription_tier: None,
        account_status: None,
    }
}

fn log_usage(
    model: &str,
    input_tokens: u64,
    output_tokens: u64,
    cache_read_tokens: u64,
) -> OAuthQuotaRequestLogUsage {
    OAuthQuotaRequestLogUsage {
        records_complete: true,
        models: vec![OAuthQuotaRequestLogModelUsage {
            public_model: model.into(),
            input_tokens,
            output_tokens,
            cache_read_tokens,
        }],
    }
}
