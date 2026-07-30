use crate::OAuthRequestPlan;

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

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthQuotaWindowKind {
    Time,
    Credits,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthQuotaWindow {
    pub id: &'static str,
    pub kind: OAuthQuotaWindowKind,
    pub used_percent: f64,
    pub limit_window_seconds: Option<u64>,
    pub reset_after_seconds: Option<u64>,
    pub reset_at: Option<i64>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthQuotaRateLimit {
    pub allowed: Option<bool>,
    pub limit_reached: Option<bool>,
    pub windows: Vec<OAuthQuotaWindow>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthQuotaResetCredit {
    pub expires_at: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthQuotaResetCredits {
    pub available_count: u32,
    pub credits: Vec<OAuthQuotaResetCredit>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OAuthQuotaBilling {
    pub currency: &'static str,
    pub prepaid_balance_minor: Option<i64>,
    pub on_demand_used_minor: Option<i64>,
    pub on_demand_cap_minor: Option<i64>,
    pub is_unified_billing_user: Option<bool>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthQuotaTokenBalanceSource {
    Upstream,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OAuthQuotaTokenBalance {
    pub source: OAuthQuotaTokenBalanceSource,
    pub used: u64,
    pub limit: u64,
    pub remaining: u64,
    pub window_seconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthQuotaAuthenticationStatus {
    Valid,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OAuthQuotaExhaustion {
    pub observed_at: i64,
    pub used: Option<u64>,
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
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

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthQuotaUsage {
    pub rate_limit: Option<OAuthQuotaRateLimit>,
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
