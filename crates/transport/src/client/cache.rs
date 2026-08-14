use std::{collections::HashMap, hash::Hash, sync::Arc};

pub(crate) struct ClientCache<K, V> {
    capacity: usize,
    tick: u64,
    entries: HashMap<K, CacheEntry<V>>,
    hits: u64,
    misses: u64,
    evictions: u64,
}

struct CacheEntry<V> {
    client: Arc<V>,
    last_used: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct ClientCacheSnapshot {
    pub(crate) entries: usize,
    pub(crate) hits: u64,
    pub(crate) misses: u64,
    pub(crate) evictions: u64,
}

impl<K, V> ClientCache<K, V>
where
    K: Clone + Eq + Hash,
{
    pub(crate) fn new(capacity: usize) -> Self {
        Self {
            capacity,
            tick: 0,
            entries: HashMap::new(),
            hits: 0,
            misses: 0,
            evictions: 0,
        }
    }

    pub(crate) fn get(&mut self, key: &K) -> Option<Arc<V>> {
        self.advance_tick();
        let tick = self.tick;
        let Some(entry) = self.entries.get_mut(key) else {
            self.misses = self.misses.saturating_add(1);
            return None;
        };
        self.hits = self.hits.saturating_add(1);
        Some({
            entry.last_used = tick;
            Arc::clone(&entry.client)
        })
    }

    /// Inserts a freshly built client unless a concurrent builder already
    /// cached one for the same key; the first cached client always wins so
    /// every caller shares one connection pool per key.
    pub(crate) fn insert_if_absent(&mut self, key: K, client: Arc<V>) -> Arc<V> {
        self.advance_tick();
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_used = self.tick;
            return Arc::clone(&entry.client);
        }
        if self.entries.len() >= self.capacity {
            let evicted = self
                .entries
                .iter()
                .min_by_key(|(_, entry)| entry.last_used)
                .map(|(key, _)| key.clone())
                .expect("a full transport cache has an entry");
            self.entries.remove(&evicted);
            self.evictions = self.evictions.saturating_add(1);
        }
        self.entries.insert(
            key,
            CacheEntry {
                client: Arc::clone(&client),
                last_used: self.tick,
            },
        );
        client
    }

    pub(crate) fn retain(&mut self, mut predicate: impl FnMut(&K) -> bool) {
        let previous_len = self.entries.len();
        self.entries.retain(|key, _| predicate(key));
        let removed = previous_len.saturating_sub(self.entries.len());
        self.evictions = self
            .evictions
            .saturating_add(u64::try_from(removed).unwrap_or(u64::MAX));
    }

    pub(crate) fn snapshot(&self) -> ClientCacheSnapshot {
        ClientCacheSnapshot {
            entries: self.entries.len(),
            hits: self.hits,
            misses: self.misses,
            evictions: self.evictions,
        }
    }

    fn advance_tick(&mut self) {
        self.tick = self
            .tick
            .checked_add(1)
            .expect("transport cache tick exhausted");
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::{ClientCache, ClientCacheSnapshot};

    #[test]
    fn snapshot_tracks_hits_misses_and_evictions() {
        let mut cache = ClientCache::new(2);

        assert!(cache.get(&"a").is_none());
        cache.insert_if_absent("a", Arc::new(1));
        assert_eq!(*cache.get(&"a").expect("cached a"), 1);
        assert!(cache.get(&"b").is_none());
        cache.insert_if_absent("b", Arc::new(2));
        assert!(cache.get(&"c").is_none());
        cache.insert_if_absent("c", Arc::new(3));
        cache.retain(|key| *key != "b");

        assert_eq!(
            cache.snapshot(),
            ClientCacheSnapshot {
                entries: 1,
                hits: 1,
                misses: 3,
                evictions: 2,
            }
        );
    }
}
