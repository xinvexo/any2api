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

impl RequestQuotaCost {
    #[must_use]
    pub fn new(
        unit: QuotaCostUnit,
        amount_nanos: u64,
        rate_card: impl Into<String>,
        service_tier: QuotaServiceTier,
    ) -> Option<Self> {
        let rate_card = rate_card.into();
        if rate_card.trim() != rate_card
            || rate_card.is_empty()
            || rate_card.chars().count() > MAX_QUOTA_RATE_CARD_CHARS
            || rate_card.chars().any(char::is_control)
        {
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
