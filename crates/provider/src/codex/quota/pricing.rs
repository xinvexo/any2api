//! Versioned OpenAI standard API prices used only for local quota observations.

use crate::api::OAuthQuotaCostRate;

pub(crate) const RATE_CARD: &str = "openai_api_standard_2026_08_11";

pub(crate) fn cost_rate(model: &str) -> Option<OAuthQuotaCostRate> {
    let (input, cached_input, output) = match model {
        "gpt-5.6-sol" | "gpt-5.5" => (5.0, 0.5, 30.0),
        "gpt-5.6-terra" => (2.0, 0.2, 12.0),
        "gpt-5.6-luna" => (0.2, 0.02, 1.2),
        "gpt-5.4" => (2.5, 0.25, 15.0),
        "gpt-5.4-mini" => (0.75, 0.075, 4.5),
        _ => return None,
    };
    Some(OAuthQuotaCostRate::new(
        RATE_CARD,
        input,
        cached_input,
        output,
    ))
}

#[cfg(test)]
mod tests {
    use any2api_domain::TokenUsage;

    use super::{RATE_CARD, cost_rate};

    #[test]
    fn standard_rate_card_prices_uncached_cached_and_output_tokens() {
        let rate = cost_rate("gpt-5.6-sol").expect("Sol rate");
        let cost = rate
            .estimate_usd(TokenUsage::new(Some(2_000), Some(100), Some(500)))
            .expect("complete usage");

        assert_eq!(rate.rate_card(), RATE_CARD);
        assert!((cost - 0.010_75).abs() < f64::EPSILON);
        assert!(cost_rate("gpt-5.3-codex-spark").is_none());
        assert!(
            rate.estimate_usd(TokenUsage::new(Some(2_000), None, None))
                .is_none()
        );
    }
}
