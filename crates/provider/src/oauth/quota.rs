use any2api_domain::{QuotaCostUnit, QuotaServiceTier, RequestQuotaCost, TokenUsage};

use crate::OAuthRequestPlan;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthQuotaRejection {
    AccountRestricted,
    ProviderEgressRestricted,
    Unclassified,
}

#[derive(Clone, Debug)]
pub struct OAuthQuotaQueryPlan {
    usage: OAuthRequestPlan,
    supplement: Option<OAuthRequestPlan>,
    reset_credits: Option<OAuthRequestPlan>,
}

impl OAuthQuotaQueryPlan {
    pub(crate) const fn new(usage: OAuthRequestPlan, reset_credits: OAuthRequestPlan) -> Self {
        Self {
            usage,
            supplement: None,
            reset_credits: Some(reset_credits),
        }
    }

    pub(crate) const fn with_supplement(
        usage: OAuthRequestPlan,
        supplement: OAuthRequestPlan,
    ) -> Self {
        Self {
            usage,
            supplement: Some(supplement),
            reset_credits: None,
        }
    }

    pub(crate) const fn without_reset_credits(usage: OAuthRequestPlan) -> Self {
        Self {
            usage,
            supplement: None,
            reset_credits: None,
        }
    }

    #[must_use]
    pub fn into_parts(
        self,
    ) -> (
        OAuthRequestPlan,
        Option<OAuthRequestPlan>,
        Option<OAuthRequestPlan>,
    ) {
        (self.usage, self.supplement, self.reset_credits)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthQuotaWindowKind {
    Time,
    Credits,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OAuthQuotaWindow {
    pub id: String,
    pub kind: OAuthQuotaWindowKind,
    pub used_percent: f64,
    pub limit_window_seconds: Option<u64>,
    pub reset_after_seconds: Option<u64>,
    pub reset_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OAuthQuotaRateLimit {
    pub allowed: Option<bool>,
    pub limit_reached: Option<bool>,
    pub windows: Vec<OAuthQuotaWindow>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OAuthQuotaCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    pub balance: Option<String>,
}

impl OAuthQuotaCredits {
    #[must_use]
    pub const fn usable(&self) -> bool {
        self.unlimited || self.has_credits
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthQuotaReachedType {
    RateLimitReached,
    WorkspaceOwnerCreditsDepleted,
    WorkspaceMemberCreditsDepleted,
    WorkspaceOwnerUsageLimitReached,
    WorkspaceMemberUsageLimitReached,
}

impl OAuthQuotaReachedType {
    #[must_use]
    pub const fn is_workspace_hard_stop(self) -> bool {
        !matches!(self, Self::RateLimitReached)
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OAuthQuotaAccessStatus {
    pub spend_control_reached: Option<bool>,
    pub reached_type: Option<OAuthQuotaReachedType>,
}

impl OAuthQuotaAccessStatus {
    #[must_use]
    pub fn workspace_hard_stop(self) -> bool {
        self.spend_control_reached == Some(true)
            || match self.reached_type {
                Some(reached) => reached.is_workspace_hard_stop(),
                None => false,
            }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct OAuthQuotaCostRate {
    rate_card: &'static str,
    unit: QuotaCostUnit,
    service_tier: QuotaServiceTier,
    input_nanos_per_million: u64,
    cached_input_nanos_per_million: u64,
    output_nanos_per_million: u64,
}

impl OAuthQuotaCostRate {
    #[must_use]
    pub const fn new(
        rate_card: &'static str,
        unit: QuotaCostUnit,
        service_tier: QuotaServiceTier,
        input_nanos_per_million: u64,
        cached_input_nanos_per_million: u64,
        output_nanos_per_million: u64,
    ) -> Self {
        Self {
            rate_card,
            unit,
            service_tier,
            input_nanos_per_million,
            cached_input_nanos_per_million,
            output_nanos_per_million,
        }
    }

    #[must_use]
    pub const fn rate_card(self) -> &'static str {
        self.rate_card
    }

    #[must_use]
    pub const fn service_tier(self) -> QuotaServiceTier {
        self.service_tier
    }

    #[must_use]
    pub fn estimate(self, usage: TokenUsage) -> Option<RequestQuotaCost> {
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
        RequestQuotaCost::new(self.unit, amount_nanos, self.rate_card, self.service_tier)
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OAuthQuotaResetCredit {
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OAuthQuotaResetCredits {
    pub available_count: u32,
    pub credits: Vec<OAuthQuotaResetCredit>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OAuthQuotaBilling {
    pub currency: String,
    pub prepaid_balance_minor: Option<i64>,
    pub on_demand_used_minor: Option<i64>,
    pub on_demand_cap_minor: Option<i64>,
    pub is_unified_billing_user: Option<bool>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthQuotaTokenBalanceSource {
    Upstream,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OAuthQuotaTokenBalance {
    pub source: OAuthQuotaTokenBalanceSource,
    pub used: u64,
    pub limit: u64,
    pub remaining: u64,
    pub window_seconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthQuotaAuthenticationStatus {
    Valid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OAuthQuotaExhaustion {
    pub observed_at: i64,
    pub used: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct OAuthQuotaAccountStatus {
    pub authentication: OAuthQuotaAuthenticationStatus,
    pub user_blocked_reason: Option<String>,
    pub team_blocked_reasons: Vec<String>,
    pub quota_exhaustion: Option<OAuthQuotaExhaustion>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OAuthQuotaSupplement {
    pub subscription_tier: Option<String>,
    pub user_blocked_reason: Option<String>,
    pub team_blocked_reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
pub struct OAuthQuotaUsage {
    pub rate_limit: Option<OAuthQuotaRateLimit>,
    pub credits: Option<OAuthQuotaCredits>,
    pub access: Option<OAuthQuotaAccessStatus>,
    pub reset_credits: Option<OAuthQuotaResetCredits>,
    pub billing: Option<OAuthQuotaBilling>,
    pub token_balance: Option<OAuthQuotaTokenBalance>,
    pub subscription_tier: Option<String>,
    pub account_status: Option<OAuthQuotaAccountStatus>,
}

impl OAuthQuotaUsage {
    pub fn replace_reset_credits(&mut self, reset_credits: OAuthQuotaResetCredits) {
        self.reset_credits = Some(reset_credits);
    }

    pub fn apply_supplement(&mut self, supplement: OAuthQuotaSupplement) {
        self.subscription_tier = supplement.subscription_tier;
        self.account_status = Some(OAuthQuotaAccountStatus {
            authentication: OAuthQuotaAuthenticationStatus::Valid,
            user_blocked_reason: supplement.user_blocked_reason,
            team_blocked_reasons: supplement.team_blocked_reasons,
            quota_exhaustion: None,
        });
    }

    pub fn replace_quota_exhaustion(&mut self, observation: Option<OAuthQuotaExhaustion>) {
        if observation.is_none() && self.account_status.is_none() {
            return;
        }
        self.account_status
            .get_or_insert_with(|| OAuthQuotaAccountStatus {
                authentication: OAuthQuotaAuthenticationStatus::Valid,
                user_blocked_reason: None,
                team_blocked_reasons: Vec::new(),
                quota_exhaustion: None,
            })
            .quota_exhaustion = observation;
    }

    pub fn replace_token_balance(&mut self, balance: Option<OAuthQuotaTokenBalance>) {
        self.token_balance = balance;
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OAuthQuotaResetResult {
    pub windows_reset: u32,
}
