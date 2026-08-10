use std::{collections::HashMap, hash::Hash, sync::Arc};

pub(crate) struct ClientCache<K, V> {
    capacity: usize,
    tick: u64,
    entries: HashMap<K, CacheEntry<V>>,
}

struct CacheEntry<V> {
    client: Arc<V>,
    last_used: u64,
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
        }
    }

    pub(crate) fn get(&mut self, key: &K) -> Option<Arc<V>> {
        self.advance_tick();
        let tick = self.tick;
        self.entries.get_mut(key).map(|entry| {
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

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }

    pub(crate) fn retain(&mut self, mut predicate: impl FnMut(&K) -> bool) {
        self.entries.retain(|key, _| predicate(key));
    }

    fn advance_tick(&mut self) {
        self.tick = self
            .tick
            .checked_add(1)
            .expect("transport cache tick exhausted");
    }
}
