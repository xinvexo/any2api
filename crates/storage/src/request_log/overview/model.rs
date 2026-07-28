use crate::error::StorageError;

pub const REQUEST_LOG_OVERVIEW_MODEL_LIMIT: usize = 12;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestLogOverviewRange {
    OneHour,
    TwentyFourHours,
    SevenDays,
    ThirtyDays,
}

impl RequestLogOverviewRange {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::OneHour => "1h",
            Self::TwentyFourHours => "24h",
            Self::SevenDays => "7d",
            Self::ThirtyDays => "30d",
        }
    }

    #[must_use]
    pub const fn bucket_width_ms(self) -> u64 {
        match self {
            Self::OneHour => 5 * 60 * 1_000,
            Self::TwentyFourHours => 60 * 60 * 1_000,
            Self::SevenDays => 6 * 60 * 60 * 1_000,
            Self::ThirtyDays => 24 * 60 * 60 * 1_000,
        }
    }

    #[must_use]
    pub const fn bucket_count(self) -> usize {
        match self {
            Self::OneHour => 12,
            Self::TwentyFourHours => 24,
            Self::SevenDays => 28,
            Self::ThirtyDays => 30,
        }
    }

    #[must_use]
    pub const fn duration_ms(self) -> u64 {
        self.bucket_width_ms() * self.bucket_count() as u64
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct RequestLogOverviewTotals {
    pub request_count: u64,
    pub successful_request_count: u64,
    pub token_usage_request_count: u64,
    pub input_tokens: u64,
    pub output_tokens: u64,
}

impl RequestLogOverviewTotals {
    #[must_use]
    pub fn failed_request_count(&self) -> u64 {
        self.request_count
            .saturating_sub(self.successful_request_count)
    }

    #[must_use]
    pub fn total_tokens(&self) -> u128 {
        u128::from(self.input_tokens) + u128::from(self.output_tokens)
    }

    pub(crate) fn checked_add(&mut self, other: &Self) -> Result<(), StorageError> {
        self.request_count = checked_add(self.request_count, other.request_count)?;
        self.successful_request_count = checked_add(
            self.successful_request_count,
            other.successful_request_count,
        )?;
        self.token_usage_request_count = checked_add(
            self.token_usage_request_count,
            other.token_usage_request_count,
        )?;
        self.input_tokens = checked_add(self.input_tokens, other.input_tokens)?;
        self.output_tokens = checked_add(self.output_tokens, other.output_tokens)?;
        Ok(())
    }

    pub(crate) fn checked_subtract(&self, other: &Self) -> Result<Self, StorageError> {
        Ok(Self {
            request_count: checked_subtract(self.request_count, other.request_count)?,
            successful_request_count: checked_subtract(
                self.successful_request_count,
                other.successful_request_count,
            )?,
            token_usage_request_count: checked_subtract(
                self.token_usage_request_count,
                other.token_usage_request_count,
            )?,
            input_tokens: checked_subtract(self.input_tokens, other.input_tokens)?,
            output_tokens: checked_subtract(self.output_tokens, other.output_tokens)?,
        })
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestLogOverviewBucket {
    pub started_at_ms: u64,
    pub ended_at_ms: u64,
    pub request_count: u64,
    pub successful_request_count: u64,
}

impl RequestLogOverviewBucket {
    #[must_use]
    pub fn failed_request_count(&self) -> u64 {
        self.request_count
            .saturating_sub(self.successful_request_count)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestLogOverviewModel {
    pub public_model: Option<String>,
    pub is_other: bool,
    pub totals: RequestLogOverviewTotals,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RequestLogOverview {
    pub generated_at_ms: u64,
    pub range: RequestLogOverviewRange,
    pub range_started_at_ms: u64,
    pub range_ended_at_ms: u64,
    pub retained_started_at_ms: Option<u64>,
    pub retained_totals: RequestLogOverviewTotals,
    pub range_totals: RequestLogOverviewTotals,
    pub time_buckets: Vec<RequestLogOverviewBucket>,
    pub models: Vec<RequestLogOverviewModel>,
}

impl RequestLogOverview {
    #[must_use]
    pub fn empty(range: RequestLogOverviewRange, now_ms: u64) -> Self {
        let range_ended_at_ms = now_ms.saturating_add(1);
        let range_started_at_ms = range_ended_at_ms.saturating_sub(range.duration_ms());
        Self {
            generated_at_ms: now_ms,
            range,
            range_started_at_ms,
            range_ended_at_ms,
            retained_started_at_ms: None,
            retained_totals: RequestLogOverviewTotals::default(),
            range_totals: RequestLogOverviewTotals::default(),
            time_buckets: empty_buckets(range, range_started_at_ms, range_ended_at_ms),
            models: Vec::new(),
        }
    }
}

pub(crate) fn empty_buckets(
    range: RequestLogOverviewRange,
    started_at_ms: u64,
    ended_at_ms: u64,
) -> Vec<RequestLogOverviewBucket> {
    (0..range.bucket_count())
        .map(|index| {
            let bucket_start = started_at_ms.saturating_add(index as u64 * range.bucket_width_ms());
            RequestLogOverviewBucket {
                started_at_ms: bucket_start,
                ended_at_ms: bucket_start
                    .saturating_add(range.bucket_width_ms())
                    .min(ended_at_ms),
                request_count: 0,
                successful_request_count: 0,
            }
        })
        .collect()
}

fn checked_add(left: u64, right: u64) -> Result<u64, StorageError> {
    left.checked_add(right)
        .ok_or(StorageError::CorruptTelemetry)
}

fn checked_subtract(left: u64, right: u64) -> Result<u64, StorageError> {
    left.checked_sub(right)
        .ok_or(StorageError::CorruptTelemetry)
}
