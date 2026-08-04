use std::{
    collections::HashMap,
    sync::{
        Mutex, MutexGuard,
        atomic::{AtomicU64, AtomicUsize, Ordering},
    },
    time::{Duration, Instant},
};

use any2api_protocol::api::{MAX_BRIDGE_CONTINUATION_STATE_BYTES, ProtocolContinuationState};

use super::{
    hash::SessionHash,
    registry::{BindingSource, BindingState, ContinuationLifecycle},
};

const SHARD_COUNT: usize = 16;

/// Upper bound on entries examined for opportunistic expiry per write so hot
/// paths never pay a table scan; the periodic sweeper removes the rest.
pub(super) const RECLAIM_SAMPLE: usize = 8;

/// Minimum spacing between whole-table reclaim scans triggered by capacity
/// rejections after a scan that found nothing to remove.
const FUTILE_RECLAIM_THROTTLE_MILLIS: u64 = 1_000;

type Shard = HashMap<SessionHash, BindingState>;

/// Session and continuation bindings split across independently locked
/// shards. Entry-count and continuation-byte budgets live in global atomic
/// counters so single-key operations take exactly one shard lock; cross-shard
/// maintenance (sweeping, snapshots, credential cleanup) visits shards one at
/// a time and therefore observes each shard at a slightly different instant.
#[derive(Debug)]
pub(super) struct ShardedBindings {
    shards: [Mutex<Shard>; SHARD_COUNT],
    max_entries: usize,
    max_continuation_bytes: usize,
    entry_count: AtomicUsize,
    continuation_bytes: AtomicUsize,
    next_version: AtomicU64,
    created_at: Instant,
    /// Milliseconds since `created_at`, offset by one, of the last full
    /// reclaim that removed nothing; zero when the last reclaim progressed.
    last_futile_reclaim: AtomicU64,
    #[cfg(test)]
    full_reclaims: AtomicUsize,
}

impl ShardedBindings {
    pub(super) fn new(max_entries: usize, max_continuation_bytes: usize) -> Self {
        Self {
            shards: std::array::from_fn(|_| Mutex::new(HashMap::new())),
            max_entries,
            max_continuation_bytes,
            entry_count: AtomicUsize::new(0),
            continuation_bytes: AtomicUsize::new(0),
            next_version: AtomicU64::new(0),
            created_at: Instant::now(),
            last_futile_reclaim: AtomicU64::new(0),
            #[cfg(test)]
            full_reclaims: AtomicUsize::new(0),
        }
    }

    pub(super) fn lock(&self, key: SessionHash) -> ShardGuard<'_> {
        self.guard(&self.shards[key.shard_index(SHARD_COUNT)])
    }

    pub(super) fn for_each_shard(&self, mut visit: impl FnMut(&mut ShardGuard<'_>)) {
        for shard in &self.shards {
            visit(&mut self.guard(shard));
        }
    }

    pub(super) fn next_version(&self) -> u64 {
        self.next_version
            .fetch_add(1, Ordering::Relaxed)
            .wrapping_add(1)
            .max(1)
    }

    /// Removes every expired entry, locking one shard at a time; used by the
    /// periodic sweeper, admin snapshots, and capacity-pressure reclaims.
    pub(super) fn remove_expired_all(&self, now: Instant, ttl: Duration) -> usize {
        let mut removed = 0;
        self.for_each_shard(|shard| removed += shard.remove_expired(now, ttl));
        removed
    }

    /// Full reclaim after a capacity rejection. Returns whether anything was
    /// removed. Scans are suppressed while a recent scan proved futile so
    /// rejections stay cheap when nothing is expirable; concurrent callers
    /// may scan twice right at the transition, which is harmless.
    pub(super) fn reclaim_for_capacity(&self, now: Instant, ttl: Duration) -> bool {
        let elapsed = self.elapsed_millis(now);
        let last_futile = self.last_futile_reclaim.load(Ordering::Relaxed);
        if last_futile != 0
            && elapsed.saturating_sub(last_futile - 1) < FUTILE_RECLAIM_THROTTLE_MILLIS
        {
            return false;
        }
        #[cfg(test)]
        self.full_reclaims.fetch_add(1, Ordering::Relaxed);
        let removed = self.remove_expired_all(now, ttl);
        let marker = if removed == 0 {
            elapsed.saturating_add(1)
        } else {
            0
        };
        self.last_futile_reclaim.store(marker, Ordering::Relaxed);
        removed > 0
    }

    fn elapsed_millis(&self, now: Instant) -> u64 {
        u64::try_from(now.saturating_duration_since(self.created_at).as_millis())
            .unwrap_or(u64::MAX)
    }

    fn guard<'a>(&'a self, shard: &'a Mutex<Shard>) -> ShardGuard<'a> {
        ShardGuard {
            owner: self,
            entries: shard.lock().expect("affinity shard lock poisoned"),
        }
    }

    fn try_reserve_entry(&self) -> bool {
        self.entry_count
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |count| {
                (count < self.max_entries).then_some(count + 1)
            })
            .is_ok()
    }

    fn try_reserve_continuation_bytes(&self, bytes: usize) -> bool {
        if bytes == 0 {
            return true;
        }
        self.continuation_bytes
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |current| {
                (current.saturating_add(bytes) <= self.max_continuation_bytes)
                    .then(|| current + bytes)
            })
            .is_ok()
    }

    #[cfg(test)]
    pub(super) fn continuation_bytes(&self) -> usize {
        self.continuation_bytes.load(Ordering::Relaxed)
    }

    #[cfg(test)]
    pub(super) fn full_reclaims(&self) -> usize {
        self.full_reclaims.load(Ordering::Relaxed)
    }
}

/// Why a new entry was rejected by the global budgets.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) enum InsertRejection {
    Entries,
    ContinuationBytes,
}

pub(super) struct ShardGuard<'a> {
    owner: &'a ShardedBindings,
    entries: MutexGuard<'a, Shard>,
}

impl ShardGuard<'_> {
    pub(super) fn get(&self, key: &SessionHash) -> Option<&BindingState> {
        self.entries.get(key)
    }

    /// Mutable access for TTL refreshes and creation-phase changes. Callers
    /// must not alter an entry's continuation footprint through this
    /// reference; footprint changes go through `replace` or `remove`.
    pub(super) fn get_mut(&mut self, key: &SessionHash) -> Option<&mut BindingState> {
        self.entries.get_mut(key)
    }

    pub(super) fn contains_key(&self, key: &SessionHash) -> bool {
        self.entries.contains_key(key)
    }

    pub(super) fn iter(&self) -> impl Iterator<Item = (&SessionHash, &BindingState)> {
        self.entries.iter()
    }

    #[cfg(test)]
    pub(super) fn iter_mut(&mut self) -> impl Iterator<Item = (&SessionHash, &mut BindingState)> {
        self.entries.iter_mut()
    }

    /// Inserts a new entry, enforcing the global entry and continuation-byte
    /// budgets. The key must not be present.
    pub(super) fn try_insert(
        &mut self,
        key: SessionHash,
        state: BindingState,
    ) -> Result<(), InsertRejection> {
        debug_assert!(!self.entries.contains_key(&key));
        if !self.owner.try_reserve_entry() {
            return Err(InsertRejection::Entries);
        }
        let bytes = continuation_footprint(&state);
        if !self.owner.try_reserve_continuation_bytes(bytes) {
            self.owner.entry_count.fetch_sub(1, Ordering::Relaxed);
            return Err(InsertRejection::ContinuationBytes);
        }
        self.entries.insert(key, state);
        Ok(())
    }

    /// Replaces an existing entry in place. Callers only shrink or keep the
    /// continuation footprint (creating -> session, pending -> ready state),
    /// so the byte budget never needs a reservation here.
    pub(super) fn replace(&mut self, key: SessionHash, state: BindingState) {
        let bytes = continuation_footprint(&state);
        let replaced = self
            .entries
            .insert(key, state)
            .expect("replace requires an existing affinity entry");
        let replaced_bytes = continuation_footprint(&replaced);
        debug_assert!(bytes <= replaced_bytes);
        if bytes < replaced_bytes {
            self.owner
                .continuation_bytes
                .fetch_sub(replaced_bytes - bytes, Ordering::Relaxed);
        } else if bytes > replaced_bytes {
            self.owner
                .continuation_bytes
                .fetch_add(bytes - replaced_bytes, Ordering::Relaxed);
        }
    }

    pub(super) fn remove(&mut self, key: &SessionHash) -> Option<BindingState> {
        let removed = self.entries.remove(key)?;
        self.owner.entry_count.fetch_sub(1, Ordering::Relaxed);
        let bytes = continuation_footprint(&removed);
        if bytes > 0 {
            self.owner
                .continuation_bytes
                .fetch_sub(bytes, Ordering::Relaxed);
        }
        Some(removed)
    }

    pub(super) fn remove_if_expired(
        &mut self,
        key: &SessionHash,
        now: Instant,
        ttl: Duration,
    ) -> bool {
        let expired = matches!(
            self.entries.get(key),
            Some(entry) if is_expired(entry, now, ttl)
        );
        if expired {
            self.remove(key);
        }
        expired
    }

    /// Removes every expired entry in this shard.
    pub(super) fn remove_expired(&mut self, now: Instant, ttl: Duration) -> usize {
        let expired = self
            .entries
            .iter()
            .filter(|(_, entry)| is_expired(entry, now, ttl))
            .map(|(key, _)| *key)
            .collect::<Vec<_>>();
        for key in &expired {
            self.remove(key);
        }
        expired.len()
    }

    /// Examines at most `RECLAIM_SAMPLE` entries of this shard and drops the
    /// expired ones, amortizing expiry across writes without a full scan.
    pub(super) fn reclaim_expired_sample(&mut self, now: Instant, ttl: Duration) {
        let mut expired = [None; RECLAIM_SAMPLE];
        let mut count = 0;
        for (key, entry) in self.entries.iter().take(RECLAIM_SAMPLE) {
            if is_expired(entry, now, ttl) {
                expired[count] = Some(*key);
                count += 1;
            }
        }
        for key in expired.iter().flatten() {
            self.remove(key);
        }
    }

    /// Drops every entry in this shard and returns how many were removed.
    pub(super) fn clear(&mut self) -> usize {
        let cleared = self.entries.len();
        let bytes = self
            .entries
            .values()
            .map(continuation_footprint)
            .sum::<usize>();
        self.entries.clear();
        self.owner.entry_count.fetch_sub(cleared, Ordering::Relaxed);
        if bytes > 0 {
            self.owner
                .continuation_bytes
                .fetch_sub(bytes, Ordering::Relaxed);
        }
        cleared
    }
}

fn is_expired(entry: &BindingState, now: Instant, ttl: Duration) -> bool {
    matches!(
        entry,
        BindingState::Bound { binding }
            if !binding.is_pending_continuation()
                && now.saturating_duration_since(binding.last_seen_at) >= ttl
    )
}

fn continuation_footprint(entry: &BindingState) -> usize {
    let BindingState::Bound { binding } = entry else {
        return 0;
    };
    match &binding.source {
        BindingSource::Session => 0,
        BindingSource::Continuation(ContinuationLifecycle::Pending { .. }) => {
            MAX_BRIDGE_CONTINUATION_STATE_BYTES
        }
        BindingSource::Continuation(ContinuationLifecycle::Ready { state }) => state
            .as_ref()
            .map_or(0, ProtocolContinuationState::serialized_bytes),
    }
}
