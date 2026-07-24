use crate::OAuthRequestPlan;

#[derive(Clone, Debug)]
pub struct OAuthQuotaQueryPlan {
    usage: OAuthRequestPlan,
    reset_credits: OAuthRequestPlan,
}

impl OAuthQuotaQueryPlan {
    pub(crate) const fn new(usage: OAuthRequestPlan, reset_credits: OAuthRequestPlan) -> Self {
        Self {
            usage,
            reset_credits,
        }
    }

    #[must_use]
    pub fn into_parts(self) -> (OAuthRequestPlan, OAuthRequestPlan) {
        (self.usage, self.reset_credits)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthQuotaWindow {
    pub used_percent: f64,
    pub limit_window_seconds: u64,
    pub reset_after_seconds: u64,
    pub reset_at: i64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthQuotaRateLimit {
    pub allowed: bool,
    pub limit_reached: bool,
    pub primary_window: Option<OAuthQuotaWindow>,
    pub secondary_window: Option<OAuthQuotaWindow>,
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

#[derive(Clone, Debug, PartialEq)]
pub struct OAuthQuotaUsage {
    pub rate_limit: Option<OAuthQuotaRateLimit>,
    pub reset_credits: Option<OAuthQuotaResetCredits>,
}

impl OAuthQuotaUsage {
    pub fn replace_reset_credits(&mut self, reset_credits: OAuthQuotaResetCredits) {
        self.reset_credits = Some(reset_credits);
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct OAuthQuotaResetResult {
    pub windows_reset: u32,
}
