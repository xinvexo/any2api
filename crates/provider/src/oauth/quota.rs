use crate::OAuthRequestPlan;

#[derive(Clone, Debug)]
pub struct OAuthQuotaQueryPlan {
    usage: OAuthRequestPlan,
    usage_probe: Option<OAuthRequestPlan>,
    reset_credits: Option<OAuthRequestPlan>,
}

impl OAuthQuotaQueryPlan {
    pub(crate) const fn new(usage: OAuthRequestPlan, reset_credits: OAuthRequestPlan) -> Self {
        Self {
            usage,
            usage_probe: None,
            reset_credits: Some(reset_credits),
        }
    }

    pub(crate) const fn with_usage_probe(
        usage: OAuthRequestPlan,
        usage_probe: OAuthRequestPlan,
    ) -> Self {
        Self {
            usage,
            usage_probe: Some(usage_probe),
            reset_credits: None,
        }
    }

    pub(crate) const fn without_reset_credits(usage: OAuthRequestPlan) -> Self {
        Self {
            usage,
            usage_probe: None,
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
        (self.usage, self.usage_probe, self.reset_credits)
    }
}

#[derive(Clone, Debug, PartialEq)]
pub enum OAuthQuotaUsageParse {
    Complete(OAuthQuotaUsage),
    ProbeRequired,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthQuotaWindowKind {
    Time,
    Credits,
    Requests,
    Tokens,
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
