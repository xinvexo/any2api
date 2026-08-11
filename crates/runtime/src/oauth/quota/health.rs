use std::time::{Duration, SystemTime, UNIX_EPOCH};

use any2api_provider::api::{OAuthQuotaReachedType, OAuthQuotaUsage, OAuthQuotaWindow};

use crate::health::CredentialHealthRuntime;

pub(super) fn synchronize(
    health: &CredentialHealthRuntime,
    usage: &OAuthQuotaUsage,
    fallback_delay: Duration,
) {
    if is_exhausted(usage) {
        let (used, limit) = exhaustion_numbers(usage);
        health.record_quota_exhaustion(reset_delay(usage).unwrap_or(fallback_delay), used, limit);
    } else if is_explicitly_available(usage) {
        health.clear_quota_exhaustion();
    }
}

fn is_exhausted(usage: &OAuthQuotaUsage) -> bool {
    workspace_hard_stop(usage)
        || (rolling_limit_exhausted(usage) && !has_usable_credits(usage))
        || usage
            .token_balance
            .is_some_and(|balance| balance.remaining == 0)
        || usage
            .account_status
            .as_ref()
            .is_some_and(|status| status.quota_exhaustion.is_some())
}

fn is_explicitly_available(usage: &OAuthQuotaUsage) -> bool {
    if workspace_hard_stop(usage)
        || usage
            .token_balance
            .is_some_and(|balance| balance.remaining == 0)
        || usage
            .account_status
            .as_ref()
            .is_some_and(|status| status.quota_exhaustion.is_some())
    {
        return false;
    }
    has_usable_credits(usage)
        || usage
            .rate_limit
            .as_ref()
            .is_some_and(|rate| rate.allowed == Some(true) || rate.limit_reached == Some(false))
        || usage
            .token_balance
            .is_some_and(|balance| balance.remaining > 0)
}

fn workspace_hard_stop(usage: &OAuthQuotaUsage) -> bool {
    usage
        .access
        .is_some_and(|access| access.workspace_hard_stop())
}

fn rolling_limit_exhausted(usage: &OAuthQuotaUsage) -> bool {
    usage
        .rate_limit
        .as_ref()
        .is_some_and(|rate| rate.allowed == Some(false) || rate.limit_reached == Some(true))
        || usage.access.as_ref().is_some_and(|access| {
            access.reached_type == Some(OAuthQuotaReachedType::RateLimitReached)
        })
}

fn has_usable_credits(usage: &OAuthQuotaUsage) -> bool {
    usage
        .credits
        .as_ref()
        .is_some_and(|credits| credits.usable())
}

fn exhaustion_numbers(usage: &OAuthQuotaUsage) -> (Option<u64>, Option<u64>) {
    if let Some(balance) = usage.token_balance.filter(|balance| balance.remaining == 0) {
        return (Some(balance.used), Some(balance.limit));
    }
    usage
        .account_status
        .as_ref()
        .and_then(|status| status.quota_exhaustion)
        .map_or((None, None), |exhaustion| {
            (exhaustion.used, exhaustion.limit)
        })
}

fn reset_delay(usage: &OAuthQuotaUsage) -> Option<Duration> {
    let rate = usage.rate_limit.as_ref()?;
    let now = unix_now();
    let saturated = rate
        .windows
        .iter()
        .filter(|window| window.used_percent >= 100.0)
        .filter_map(|window| window_reset_delay(window, now))
        .min();
    saturated
        .or_else(|| {
            rate.windows
                .iter()
                .filter_map(|window| window_reset_delay(window, now))
                .min()
        })
        .map(Duration::from_secs)
}

fn window_reset_delay(window: &OAuthQuotaWindow, now: i64) -> Option<u64> {
    window.reset_after_seconds.or_else(|| {
        window
            .reset_at
            .map(|reset_at| u64::try_from(reset_at.saturating_sub(now)).unwrap_or_default())
    })
}

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|duration| i64::try_from(duration.as_secs()).ok())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use any2api_provider::api::{
        OAuthQuotaAccessStatus, OAuthQuotaCredits, OAuthQuotaRateLimit, OAuthQuotaReachedType,
        OAuthQuotaUsage, OAuthQuotaWindow, OAuthQuotaWindowKind,
    };

    use super::{is_exhausted, is_explicitly_available};

    #[test]
    fn a_percent_only_window_does_not_decide_account_availability() {
        let usage = OAuthQuotaUsage {
            rate_limit: Some(OAuthQuotaRateLimit {
                allowed: None,
                limit_reached: None,
                windows: vec![OAuthQuotaWindow {
                    id: "model_window".to_owned(),
                    kind: OAuthQuotaWindowKind::Time,
                    used_percent: 100.0,
                    limit_window_seconds: Some(3_600),
                    reset_after_seconds: Some(60),
                    reset_at: None,
                }],
            }),
            credits: None,
            access: None,
            reset_credits: None,
            billing: None,
            token_balance: None,
            subscription_tier: None,
            account_status: None,
        };

        assert!(!is_exhausted(&usage));
        assert!(!is_explicitly_available(&usage));
    }

    #[test]
    fn real_credits_keep_rolling_exhaustion_available() {
        let usage = usage_with_credits_and_access(
            OAuthQuotaCredits {
                has_credits: true,
                unlimited: false,
                balance: Some("17.5".to_owned()),
            },
            OAuthQuotaAccessStatus {
                spend_control_reached: Some(false),
                reached_type: Some(OAuthQuotaReachedType::RateLimitReached),
            },
        );

        assert!(!is_exhausted(&usage));
        assert!(is_explicitly_available(&usage));
    }

    #[test]
    fn workspace_hard_stop_wins_over_real_credits() {
        let usage = usage_with_credits_and_access(
            OAuthQuotaCredits {
                has_credits: true,
                unlimited: false,
                balance: Some("17.5".to_owned()),
            },
            OAuthQuotaAccessStatus {
                spend_control_reached: Some(false),
                reached_type: Some(OAuthQuotaReachedType::WorkspaceMemberUsageLimitReached),
            },
        );

        assert!(is_exhausted(&usage));
        assert!(!is_explicitly_available(&usage));
    }

    fn usage_with_credits_and_access(
        credits: OAuthQuotaCredits,
        access: OAuthQuotaAccessStatus,
    ) -> OAuthQuotaUsage {
        OAuthQuotaUsage {
            rate_limit: Some(OAuthQuotaRateLimit {
                allowed: Some(false),
                limit_reached: Some(true),
                windows: Vec::new(),
            }),
            credits: Some(credits),
            access: Some(access),
            reset_credits: None,
            billing: None,
            token_balance: None,
            subscription_tier: None,
            account_status: None,
        }
    }
}
