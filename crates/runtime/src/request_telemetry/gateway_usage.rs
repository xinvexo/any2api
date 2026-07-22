use std::{
    collections::HashMap,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use any2api_domain::GatewayApiKeyId;

const LAST_USED_THROTTLE: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(super) struct GatewayUsageTracker {
    live_last_used_at: HashMap<GatewayApiKeyId, String>,
    last_enqueued_at: HashMap<GatewayApiKeyId, Instant>,
}

impl GatewayUsageTracker {
    pub(super) fn observe(&mut self, id: GatewayApiKeyId, used_at: String, now: Instant) -> bool {
        self.live_last_used_at.insert(id, used_at);
        match self.last_enqueued_at.get(&id).copied() {
            Some(previous) if now.duration_since(previous) < LAST_USED_THROTTLE => false,
            _ => {
                self.last_enqueued_at.insert(id, now);
                true
            }
        }
    }

    pub(super) fn last_used_at(&self, id: GatewayApiKeyId) -> Option<String> {
        self.live_last_used_at.get(&id).cloned()
    }
}

pub(super) fn utc_timestamp() -> String {
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default();
    let secs = now.as_secs();
    let days = secs / 86_400;
    let time = secs % 86_400;
    let (year, month, day) = civil_from_days(i64::try_from(days).unwrap_or(i64::MAX));
    let hour = time / 3_600;
    let minute = (time % 3_600) / 60;
    let second = time % 60;
    format!("{year:04}-{month:02}-{day:02} {hour:02}:{minute:02}:{second:02}")
}

// Howard Hinnant civil_from_days for proleptic Gregorian UTC dates.
fn civil_from_days(days: i64) -> (i32, u32, u32) {
    let z = days + 719_468;
    let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
    let doe = z - era * 146_097;
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    (
        i32::try_from(y).unwrap_or(i32::MAX),
        u32::try_from(m).unwrap_or(u32::MAX),
        u32::try_from(d).unwrap_or(u32::MAX),
    )
}

#[cfg(test)]
mod tests {
    use std::time::{Duration, Instant};

    use any2api_domain::GatewayApiKeyId;

    use super::{GatewayUsageTracker, LAST_USED_THROTTLE};

    #[test]
    fn usage_is_live_immediately_and_persisted_at_most_once_per_interval() {
        let id = GatewayApiKeyId::new();
        let started = Instant::now();
        let mut tracker = GatewayUsageTracker::default();

        assert!(tracker.observe(id, "2026-07-22 10:00:00".into(), started));
        assert!(!tracker.observe(
            id,
            "2026-07-22 10:00:59".into(),
            started + LAST_USED_THROTTLE - Duration::from_millis(1),
        ));
        assert_eq!(
            tracker.last_used_at(id).as_deref(),
            Some("2026-07-22 10:00:59")
        );
        assert!(tracker.observe(
            id,
            "2026-07-22 10:01:00".into(),
            started + LAST_USED_THROTTLE,
        ));
    }
}
