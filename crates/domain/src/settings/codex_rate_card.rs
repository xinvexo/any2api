use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::{
    QuotaCostUnit, QuotaServiceTier, RequestQuotaCostRate, RequestQuotaCostRates,
    SettingsValidationError, UpstreamModelName,
};

pub const MAX_CODEX_RATE_CARD_MODELS: usize = 256;
pub const MAX_CODEX_RATE_NANOS_PER_MILLION: u64 = 9_000_000_000_000_000;
pub const MAX_CODEX_CREDITS_PER_USD: u64 = 1_000_000;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodexQuotaTierRate {
    pub input_nanos_per_million: u64,
    pub cached_input_nanos_per_million: u64,
    pub output_nanos_per_million: u64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodexQuotaModelRates {
    pub standard: CodexQuotaTierRate,
    #[serde(default)]
    pub fast: Option<CodexQuotaTierRate>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct CodexQuotaRateCard {
    pub id: String,
    pub credits_per_usd: u64,
    pub models: BTreeMap<String, CodexQuotaModelRates>,
}

impl CodexQuotaRateCard {
    pub fn from_json(value: &Value) -> Result<Self, SettingsValidationError> {
        let card: Self = serde_json::from_value(value.clone())
            .map_err(|_| SettingsValidationError::InvalidType)?;
        card.validate()?;
        Ok(card)
    }

    pub fn to_json(&self) -> Value {
        serde_json::to_value(self).expect("validated Codex rate card is serializable")
    }

    pub fn validate(&self) -> Result<(), SettingsValidationError> {
        if self.id.trim() != self.id
            || self.id.is_empty()
            || self.id.chars().count() > 128
            || self.id.chars().any(char::is_control)
            || self.credits_per_usd == 0
            || self.credits_per_usd > MAX_CODEX_CREDITS_PER_USD
            || self.models.is_empty()
            || self.models.len() > MAX_CODEX_RATE_CARD_MODELS
        {
            return Err(SettingsValidationError::InvalidCombination);
        }
        for (model, rates) in &self.models {
            UpstreamModelName::new(model.clone())
                .map_err(|_| SettingsValidationError::InvalidListValue)?;
            validate_tier(rates.standard)?;
            if let Some(fast) = rates.fast {
                validate_tier(fast)?;
            }
        }
        Ok(())
    }

    pub fn rate(&self, model: &str, service_tier: QuotaServiceTier) -> Option<CodexQuotaTierRate> {
        let rates = self.models.get(model)?;
        match service_tier {
            QuotaServiceTier::Standard => Some(rates.standard),
            QuotaServiceTier::Fast => rates.fast,
        }
    }

    pub fn cost_rate(
        &self,
        model: &str,
        service_tier: QuotaServiceTier,
    ) -> Option<RequestQuotaCostRate> {
        let rate = self.rate(model, service_tier)?;
        RequestQuotaCostRate::new(
            self.id.clone(),
            QuotaCostUnit::CodexCredits,
            service_tier,
            rate.input_nanos_per_million,
            rate.cached_input_nanos_per_million,
            rate.output_nanos_per_million,
        )
    }

    pub fn cost_rates(&self, model: &str) -> Option<RequestQuotaCostRates> {
        RequestQuotaCostRates::new(
            self.cost_rate(model, QuotaServiceTier::Standard)?,
            self.cost_rate(model, QuotaServiceTier::Fast),
        )
    }

    pub fn id(&self) -> &str {
        &self.id
    }

    pub const fn credits_per_usd(&self) -> u64 {
        self.credits_per_usd
    }
}

impl Default for CodexQuotaRateCard {
    fn default() -> Self {
        let mut models = BTreeMap::new();
        models.insert(
            "gpt-5.6-sol".to_owned(),
            model(
                (125_000_000_000, 12_500_000_000, 750_000_000_000),
                (312_500_000_000, 31_250_000_000, 1_875_000_000_000),
            ),
        );
        models.insert(
            "gpt-5.5".to_owned(),
            model(
                (125_000_000_000, 12_500_000_000, 750_000_000_000),
                (312_500_000_000, 31_250_000_000, 1_875_000_000_000),
            ),
        );
        models.insert(
            "gpt-5.6-terra".to_owned(),
            model(
                (50_000_000_000, 5_000_000_000, 300_000_000_000),
                (125_000_000_000, 12_500_000_000, 750_000_000_000),
            ),
        );
        models.insert(
            "gpt-5.6-luna".to_owned(),
            model(
                (5_000_000_000, 500_000_000, 30_000_000_000),
                (12_500_000_000, 1_250_000_000, 75_000_000_000),
            ),
        );
        models.insert(
            "gpt-5.4".to_owned(),
            model(
                (62_500_000_000, 6_250_000_000, 375_000_000_000),
                (125_000_000_000, 12_500_000_000, 750_000_000_000),
            ),
        );
        models.insert(
            "gpt-5.4-mini".to_owned(),
            model(
                (18_750_000_000, 1_875_000_000, 113_000_000_000),
                (37_500_000_000, 3_750_000_000, 226_000_000_000),
            ),
        );
        Self {
            id: "openai_codex_credits_2026_08_11".to_owned(),
            credits_per_usd: 25,
            models,
        }
    }
}

fn model(standard: (u64, u64, u64), fast: (u64, u64, u64)) -> CodexQuotaModelRates {
    CodexQuotaModelRates {
        standard: tier(standard),
        fast: Some(tier(fast)),
    }
}

fn tier((input, cached_input, output): (u64, u64, u64)) -> CodexQuotaTierRate {
    CodexQuotaTierRate {
        input_nanos_per_million: input,
        cached_input_nanos_per_million: cached_input,
        output_nanos_per_million: output,
    }
}

fn validate_tier(rate: CodexQuotaTierRate) -> Result<(), SettingsValidationError> {
    if rate.input_nanos_per_million == 0
        || rate.output_nanos_per_million == 0
        || rate.cached_input_nanos_per_million > rate.input_nanos_per_million
        || rate.input_nanos_per_million > MAX_CODEX_RATE_NANOS_PER_MILLION
        || rate.cached_input_nanos_per_million > MAX_CODEX_RATE_NANOS_PER_MILLION
        || rate.output_nanos_per_million > MAX_CODEX_RATE_NANOS_PER_MILLION
    {
        return Err(SettingsValidationError::InvalidCombination);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::{CodexQuotaRateCard, MAX_CODEX_RATE_NANOS_PER_MILLION};
    use crate::{QuotaServiceTier, SettingKey, SettingValue};

    #[test]
    fn default_card_matches_the_previous_standard_and_fast_rates() {
        let card = CodexQuotaRateCard::default();
        assert_eq!(card.credits_per_usd(), 25);
        assert_eq!(
            card.rate("gpt-5.6-sol", QuotaServiceTier::Standard)
                .expect("standard")
                .output_nanos_per_million,
            750_000_000_000
        );
        assert_eq!(
            card.rate("gpt-5.6-sol", QuotaServiceTier::Fast)
                .expect("fast")
                .input_nanos_per_million,
            312_500_000_000
        );
        let cost = card
            .cost_rates("gpt-5.6-sol")
            .expect("model cost rates")
            .estimate(
                crate::TokenUsage::new(Some(2_000), Some(100), Some(500)),
                None,
            )
            .expect("complete usage");
        assert_eq!(cost.amount_nanos, 268_750_000);
        assert_eq!(cost.rate_card, card.id());
        assert_eq!(cost.service_tier, QuotaServiceTier::Standard);

        let fast = card
            .cost_rates("gpt-5.6-sol")
            .expect("model cost rates")
            .estimate(
                crate::TokenUsage::new(Some(2_000), Some(100), Some(500)),
                Some(crate::RequestSpeedTier::Fast),
            )
            .expect("complete usage");
        assert_eq!(fast.amount_nanos, 671_875_000);
        assert_eq!(fast.service_tier, QuotaServiceTier::Fast);
    }

    #[test]
    fn schema_rejects_unknown_fields_and_invalid_rates() {
        let mut value = CodexQuotaRateCard::default().to_json();
        value["extra"] = json!(true);
        assert!(CodexQuotaRateCard::from_json(&value).is_err());
        let mut value = CodexQuotaRateCard::default().to_json();
        value["models"]["gpt-5.6-sol"]["standard"]["input_nanos_per_million"] =
            json!(MAX_CODEX_RATE_NANOS_PER_MILLION + 1);
        assert!(CodexQuotaRateCard::from_json(&value).is_err());
    }

    #[test]
    fn setting_value_round_trips_the_card() {
        let value = SettingValue::from_json(
            SettingKey::OAuthCodexRateCard,
            &CodexQuotaRateCard::default().to_json(),
        )
        .expect("rate card");
        assert_eq!(value.to_json(), CodexQuotaRateCard::default().to_json());
    }
}
