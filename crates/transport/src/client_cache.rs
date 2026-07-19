use std::{collections::HashMap, hash::Hash, sync::Arc};

use reqwest::Client;

pub(crate) struct ClientCache<K> {
    capacity: usize,
    tick: u64,
    entries: HashMap<K, CacheEntry>,
}

struct CacheEntry {
    client: Arc<Client>,
    last_used: u64,
}

impl<K> ClientCache<K>
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

    pub(crate) fn get_or_insert_with<F, E>(&mut self, key: K, build: F) -> Result<Arc<Client>, E>
    where
        F: FnOnce() -> Result<Client, E>,
    {
        self.tick = self
            .tick
            .checked_add(1)
            .expect("transport cache tick exhausted");
        if let Some(entry) = self.entries.get_mut(&key) {
            entry.last_used = self.tick;
            return Ok(Arc::clone(&entry.client));
        }

        let client = Arc::new(build()?);
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
        Ok(client)
    }

    pub(crate) fn len(&self) -> usize {
        self.entries.len()
    }
}
