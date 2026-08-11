//! Versioned official Codex Credits rates used for local quota observations.

use crate::api::OAuthQuotaCostRate;
use any2api_domain::{QuotaCostUnit, QuotaServiceTier};

pub(crate) const RATE_CARD: &str = "openai_codex_credits_2026_08_11";

pub(crate) fn cost_rate(model: &str, service_tier: QuotaServiceTier) -> Option<OAuthQuotaCostRate> {
    let (input, cached_input, output) = match model {
        "gpt-5.6-sol" | "gpt-5.5" => (125_000_000_000, 12_500_000_000, 750_000_000_000),
        "gpt-5.6-terra" => (50_000_000_000, 5_000_000_000, 300_000_000_000),
        "gpt-5.6-luna" => (5_000_000_000, 500_000_000, 30_000_000_000),
        "gpt-5.4" => (62_500_000_000, 6_250_000_000, 375_000_000_000),
        "gpt-5.4-mini" => (18_750_000_000, 1_875_000_000, 113_000_000_000),
        _ => return None,
    };
    let multiplier = match (model, service_tier) {
        (_, QuotaServiceTier::Standard) => (1, 1),
        ("gpt-5.6-sol" | "gpt-5.6-terra" | "gpt-5.6-luna" | "gpt-5.5", QuotaServiceTier::Fast) => {
            (5, 2)
        }
        ("gpt-5.4" | "gpt-5.4-mini", QuotaServiceTier::Fast) => (2, 1),
        _ => return None,
    };
    Some(OAuthQuotaCostRate::new(
        RATE_CARD,
        QuotaCostUnit::CodexCredits,
        service_tier,
        apply_multiplier(input, multiplier)?,
        apply_multiplier(cached_input, multiplier)?,
        apply_multiplier(output, multiplier)?,
    ))
}

fn apply_multiplier(value: u64, (numerator, denominator): (u64, u64)) -> Option<u64> {
    value.checked_mul(numerator)?.checked_div(denominator)
}

#[cfg(test)]
mod tests {
    use any2api_domain::{QuotaServiceTier, TokenUsage};

    use super::{RATE_CARD, cost_rate};

    #[test]
    fn credit_rate_card_prices_standard_and_fast_usage() {
        let rate = cost_rate("gpt-5.6-sol", QuotaServiceTier::Standard).expect("Sol rate");
        let cost = rate
            .estimate(TokenUsage::new(Some(2_000), Some(100), Some(500)))
            .expect("complete usage");

        assert_eq!(rate.rate_card(), RATE_CARD);
        assert_eq!(cost.amount_nanos, 268_750_000);
        let fast = cost_rate("gpt-5.6-sol", QuotaServiceTier::Fast)
            .expect("fast Sol rate")
            .estimate(TokenUsage::new(Some(2_000), Some(100), Some(500)))
            .expect("fast cost");
        assert_eq!(fast.amount_nanos, 671_875_000);
        assert!(cost_rate("gpt-5.3-codex-spark", QuotaServiceTier::Standard).is_none());
        assert!(
            rate.estimate(TokenUsage::new(Some(2_000), None, None))
                .is_none()
        );
    }
}
