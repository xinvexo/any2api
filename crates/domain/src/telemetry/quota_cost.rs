use super::TokenUsage;

pub const MAX_QUOTA_RATE_CARD_CHARS: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaCostUnit {
    CodexCredits,
}

impl QuotaCostUnit {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::CodexCredits => "codex_credits",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "codex_credits" => Some(Self::CodexCredits),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum QuotaServiceTier {
    Standard,
    Fast,
}

impl QuotaServiceTier {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Fast => "fast",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(Self::Standard),
            "fast" => Some(Self::Fast),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestQuotaCost {
    pub unit: QuotaCostUnit,
    pub amount_nanos: u64,
    pub rate_card: String,
    pub service_tier: QuotaServiceTier,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestQuotaCostRate {
    rate_card: String,
    unit: QuotaCostUnit,
    service_tier: QuotaServiceTier,
    input_nanos_per_million: u64,
    cached_input_nanos_per_million: u64,
    output_nanos_per_million: u64,
}

impl RequestQuotaCostRate {
    #[must_use]
    pub fn new(
        rate_card: impl Into<String>,
        unit: QuotaCostUnit,
        service_tier: QuotaServiceTier,
        input_nanos_per_million: u64,
        cached_input_nanos_per_million: u64,
        output_nanos_per_million: u64,
    ) -> Option<Self> {
        let rate_card = rate_card.into();
        valid_rate_card(&rate_card).then_some(Self {
            rate_card,
            unit,
            service_tier,
            input_nanos_per_million,
            cached_input_nanos_per_million,
            output_nanos_per_million,
        })
    }

    pub fn rate_card(&self) -> &str {
        &self.rate_card
    }

    pub const fn service_tier(&self) -> QuotaServiceTier {
        self.service_tier
    }

    #[must_use]
    pub fn estimate(&self, usage: TokenUsage) -> Option<RequestQuotaCost> {
        let input = usage.input_tokens()?;
        let output = usage.output_tokens()?;
        let cached = usage.cache_read_tokens().unwrap_or_default().min(input);
        let uncached = input.saturating_sub(cached);
        let numerator = u128::from(uncached)
            .checked_mul(u128::from(self.input_nanos_per_million))?
            .checked_add(
                u128::from(cached).checked_mul(u128::from(self.cached_input_nanos_per_million))?,
            )?
            .checked_add(
                u128::from(output).checked_mul(u128::from(self.output_nanos_per_million))?,
            )?;
        let rounded = numerator.checked_add(500_000)?.checked_div(1_000_000)?;
        let amount_nanos = u64::try_from(rounded).ok()?;
        if amount_nanos > i64::MAX as u64 {
            return None;
        }
        RequestQuotaCost::new(
            self.unit,
            amount_nanos,
            self.rate_card.clone(),
            self.service_tier,
        )
    }
}

impl RequestQuotaCost {
    #[must_use]
    pub fn new(
        unit: QuotaCostUnit,
        amount_nanos: u64,
        rate_card: impl Into<String>,
        service_tier: QuotaServiceTier,
    ) -> Option<Self> {
        let rate_card = rate_card.into();
        if !valid_rate_card(&rate_card) {
            return None;
        }
        Some(Self {
            unit,
            amount_nanos,
            rate_card,
            service_tier,
        })
    }
}

fn valid_rate_card(value: &str) -> bool {
    value.trim() == value
        && !value.is_empty()
        && value.chars().count() <= MAX_QUOTA_RATE_CARD_CHARS
        && !value.chars().any(char::is_control)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quota_cost_metadata_is_bounded_and_stable() {
        assert!(
            RequestQuotaCost::new(
                QuotaCostUnit::CodexCredits,
                42,
                "codex_credits_2026_08_11",
                QuotaServiceTier::Fast,
            )
            .is_some()
        );
        assert!(
            RequestQuotaCost::new(
                QuotaCostUnit::CodexCredits,
                42,
                " invalid",
                QuotaServiceTier::Standard,
            )
            .is_none()
        );
    }
}
