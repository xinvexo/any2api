use std::{
    collections::{HashMap, HashSet},
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};

use any2api_domain::{ConfigRevision, GatewayApiKeyId};

const LAST_USED_THROTTLE: Duration = Duration::from_secs(60);

#[derive(Default)]
pub(super) struct GatewayUsageTracker {
    live_last_used_at: HashMap<GatewayApiKeyId, String>,
    last_enqueued_at: HashMap<GatewayApiKeyId, Instant>,
    reconciled_revision: Option<ConfigRevision>,
    active_ids: HashSet<GatewayApiKeyId>,
}

impl GatewayUsageTracker {
    pub(super) fn observe(
        &mut self,
        id: GatewayApiKeyId,
        revision: ConfigRevision,
        used_at: String,
        now: Instant,
    ) -> bool {
        if self
            .reconciled_revision
            .is_some_and(|current| revision <= current)
            && !self.active_ids.contains(&id)
        {
            return false;
        }
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

    pub(super) fn reconcile(
        &mut self,
        revision: ConfigRevision,
        active_ids: impl IntoIterator<Item = GatewayApiKeyId>,
    ) {
        if self
            .reconciled_revision
            .is_some_and(|current| revision < current)
        {
            return;
        }
        let active_ids = active_ids.into_iter().collect::<HashSet<_>>();
        self.live_last_used_at
            .retain(|id, _| active_ids.contains(id));
        self.last_enqueued_at
            .retain(|id, _| active_ids.contains(id));
        self.live_last_used_at.shrink_to_fit();
        self.last_enqueued_at.shrink_to_fit();
        self.reconciled_revision = Some(revision);
        self.active_ids = active_ids;
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

    use any2api_domain::{ConfigRevision, GatewayApiKeyId};

    use super::{GatewayUsageTracker, LAST_USED_THROTTLE};

    #[test]
    #[ignore = "manual Gateway API Key churn benchmark"]
    fn deleted_gateway_key_churn_benchmark() {
        const CHURNED_KEYS: usize = 100_000;

        let started = Instant::now();
        let mut tracker = GatewayUsageTracker::default();
        for offset in 0..CHURNED_KEYS {
            tracker.observe(
                GatewayApiKeyId::new(),
                ConfigRevision::INITIAL,
                format!("2026-08-04 00:00:{:02}", offset % 60),
                started,
            );
        }
        let insert_elapsed = started.elapsed();
        let live_capacity_before = tracker.live_last_used_at.capacity();
        let throttle_capacity_before = tracker.last_enqueued_at.capacity();
        let reconcile_started = Instant::now();
        tracker.reconcile(
            ConfigRevision::new(2).expect("next revision"),
            std::iter::empty(),
        );
        let reconcile_elapsed = reconcile_started.elapsed();

        eprintln!(
            "gateway usage churn: keys={CHURNED_KEYS}, live_capacity_before={live_capacity_before}, throttle_capacity_before={throttle_capacity_before}, live_len_after={}, live_capacity_after={}, throttle_len_after={}, throttle_capacity_after={}, insert_elapsed={insert_elapsed:?}, reconcile_elapsed={reconcile_elapsed:?}",
            tracker.live_last_used_at.len(),
            tracker.live_last_used_at.capacity(),
            tracker.last_enqueued_at.len(),
            tracker.last_enqueued_at.capacity(),
        );
        assert_eq!(tracker.live_last_used_at.len(), 0);
        assert_eq!(tracker.live_last_used_at.capacity(), 0);
        assert_eq!(tracker.last_enqueued_at.len(), 0);
        assert_eq!(tracker.last_enqueued_at.capacity(), 0);
    }

    #[test]
    fn usage_is_live_immediately_and_persisted_at_most_once_per_interval() {
        let id = GatewayApiKeyId::new();
        let started = Instant::now();
        let mut tracker = GatewayUsageTracker::default();

        assert!(tracker.observe(
            id,
            ConfigRevision::INITIAL,
            "2026-07-22 10:00:00".into(),
            started
        ));
        assert!(!tracker.observe(
            id,
            ConfigRevision::INITIAL,
            "2026-07-22 10:00:59".into(),
            started + LAST_USED_THROTTLE - Duration::from_millis(1),
        ));
        assert_eq!(
            tracker.last_used_at(id).as_deref(),
            Some("2026-07-22 10:00:59")
        );
        assert!(tracker.observe(
            id,
            ConfigRevision::INITIAL,
            "2026-07-22 10:01:00".into(),
            started + LAST_USED_THROTTLE,
        ));
    }

    #[test]
    fn reconcile_prunes_deleted_keys_and_blocks_late_old_revision_reinsertion() {
        let kept = GatewayApiKeyId::new();
        let deleted = GatewayApiKeyId::new();
        let new_before_reconcile = GatewayApiKeyId::new();
        let started = Instant::now();
        let revision_two = ConfigRevision::new(2).expect("revision two");
        let revision_three = ConfigRevision::new(3).expect("revision three");
        let mut tracker = GatewayUsageTracker::default();

        assert!(tracker.observe(
            kept,
            ConfigRevision::INITIAL,
            "2026-08-04 00:00:00".into(),
            started,
        ));
        assert!(tracker.observe(
            deleted,
            ConfigRevision::INITIAL,
            "2026-08-04 00:00:00".into(),
            started,
        ));
        tracker.reconcile(revision_two, [kept]);

        assert!(tracker.last_used_at(kept).is_some());
        assert!(tracker.last_used_at(deleted).is_none());
        assert!(!tracker.observe(
            deleted,
            ConfigRevision::INITIAL,
            "2026-08-04 00:00:01".into(),
            started + Duration::from_secs(61),
        ));
        assert!(tracker.observe(
            new_before_reconcile,
            revision_three,
            "2026-08-04 00:00:01".into(),
            started + Duration::from_secs(61),
        ));
        tracker.reconcile(revision_three, [kept, new_before_reconcile]);
        tracker.reconcile(revision_two, std::iter::empty());

        assert!(tracker.last_used_at(kept).is_some());
        assert!(tracker.last_used_at(new_before_reconcile).is_some());
        assert!(tracker.last_used_at(deleted).is_none());
        assert_eq!(tracker.live_last_used_at.len(), 2);
        assert_eq!(tracker.last_enqueued_at.len(), 2);
    }
}
