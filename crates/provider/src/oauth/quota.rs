use crate::OAuthRequestPlan;
use serde::{Deserialize, Deserializer, Serialize};

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
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaWindow {
    pub id: String,
    pub kind: OAuthQuotaWindowKind,
    pub used_percent: f64,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub limit_window_seconds: Option<u64>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub reset_after_seconds: Option<u64>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub reset_at: Option<i64>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaRateLimit {
    #[serde(deserialize_with = "deserialize_nullable")]
    pub allowed: Option<bool>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub limit_reached: Option<bool>,
    pub windows: Vec<OAuthQuotaWindow>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaCredits {
    pub has_credits: bool,
    pub unlimited: bool,
    #[serde(deserialize_with = "deserialize_nullable")]
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
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaAccessStatus {
    #[serde(deserialize_with = "deserialize_nullable")]
    pub spend_control_reached: Option<bool>,
    #[serde(deserialize_with = "deserialize_nullable")]
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

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaResetCredit {
    pub expires_at: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaResetCredits {
    pub available_count: u32,
    pub credits: Vec<OAuthQuotaResetCredit>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaBilling {
    pub currency: String,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub prepaid_balance_minor: Option<i64>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub on_demand_used_minor: Option<i64>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub on_demand_cap_minor: Option<i64>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub is_unified_billing_user: Option<bool>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthQuotaTokenBalanceSource {
    Upstream,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaTokenBalance {
    pub source: OAuthQuotaTokenBalanceSource,
    pub used: u64,
    pub limit: u64,
    pub remaining: u64,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub window_seconds: Option<u64>,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum OAuthQuotaAuthenticationStatus {
    Valid,
}

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaExhaustion {
    pub observed_at: i64,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub used: Option<u64>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub limit: Option<u64>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaAccountStatus {
    pub authentication: OAuthQuotaAuthenticationStatus,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub user_blocked_reason: Option<String>,
    pub team_blocked_reasons: Vec<String>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub quota_exhaustion: Option<OAuthQuotaExhaustion>,
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct OAuthQuotaSupplement {
    pub subscription_tier: Option<String>,
    pub user_blocked_reason: Option<String>,
    pub team_blocked_reasons: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct OAuthQuotaUsage {
    #[serde(deserialize_with = "deserialize_nullable")]
    pub rate_limit: Option<OAuthQuotaRateLimit>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub credits: Option<OAuthQuotaCredits>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub access: Option<OAuthQuotaAccessStatus>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub reset_credits: Option<OAuthQuotaResetCredits>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub billing: Option<OAuthQuotaBilling>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub token_balance: Option<OAuthQuotaTokenBalance>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub subscription_tier: Option<String>,
    #[serde(deserialize_with = "deserialize_nullable")]
    pub account_status: Option<OAuthQuotaAccountStatus>,
}

fn deserialize_nullable<'de, D, T>(deserializer: D) -> Result<Option<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    Option::<T>::deserialize(deserializer)
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
