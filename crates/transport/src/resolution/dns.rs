use std::{
    collections::HashMap,
    net::IpAddr,
    sync::{Arc, LazyLock, Mutex},
    time::Duration,
};

use tokio::{net::lookup_host, time::Instant};

const DNS_CACHE_TTL: Duration = Duration::from_secs(30);
const DNS_CACHE_CAPACITY: usize = 1024;

/// Process-wide DNS cache shared by every transport client so per-request
/// lookups amortize to a hash-map read within the TTL window.
pub(crate) fn shared_dns_cache() -> &'static DnsCache {
    static CACHE: LazyLock<DnsCache> = LazyLock::new(DnsCache::new);
    &CACHE
}

#[derive(Clone, Debug, thiserror::Error)]
pub(crate) enum DnsLookupError {
    #[error("upstream DNS resolution failed")]
    Failed,
    #[error("upstream DNS resolution returned no addresses")]
    Empty,
}

pub(crate) struct DnsCache {
    entries: Mutex<HashMap<Arc<str>, DnsCacheEntry>>,
}

struct DnsCacheEntry {
    addresses: Arc<[IpAddr]>,
    expires_at: Instant,
}

impl DnsCache {
    fn new() -> Self {
        Self {
            entries: Mutex::new(HashMap::new()),
        }
    }

    /// Resolves a host to a non-empty, deterministically ordered address list.
    /// IP literals bypass the cache; hostnames are cached for the TTL.
    pub(crate) async fn resolve(&self, host: &str) -> Result<Arc<[IpAddr]>, DnsLookupError> {
        if let Ok(address) = host.parse::<IpAddr>() {
            return Ok(Arc::from(vec![address].into_boxed_slice()));
        }
        if let Some(addresses) = self.cached(host) {
            return Ok(addresses);
        }
        let mut addresses = lookup_host((host, 0_u16))
            .await
            .map_err(|_| DnsLookupError::Failed)?
            .map(|address| address.ip())
            .collect::<Vec<_>>();
        addresses.sort_unstable();
        addresses.dedup();
        if addresses.is_empty() {
            return Err(DnsLookupError::Empty);
        }
        let addresses: Arc<[IpAddr]> = Arc::from(addresses.into_boxed_slice());
        self.store(host, Arc::clone(&addresses));
        Ok(addresses)
    }

    fn cached(&self, host: &str) -> Option<Arc<[IpAddr]>> {
        let entries = self.entries.lock().expect("DNS cache lock poisoned");
        let entry = entries.get(host)?;
        (entry.expires_at > Instant::now()).then(|| Arc::clone(&entry.addresses))
    }

    fn store(&self, host: &str, addresses: Arc<[IpAddr]>) {
        let mut entries = self.entries.lock().expect("DNS cache lock poisoned");
        if entries.len() >= DNS_CACHE_CAPACITY && !entries.contains_key(host) {
            let now = Instant::now();
            entries.retain(|_, entry| entry.expires_at > now);
            if entries.len() >= DNS_CACHE_CAPACITY {
                let evicted = entries
                    .iter()
                    .min_by_key(|(_, entry)| entry.expires_at)
                    .map(|(host, _)| Arc::clone(host))
                    .expect("a full DNS cache has an entry");
                entries.remove(&evicted);
            }
        }
        entries.insert(
            Arc::from(host.to_owned()),
            DnsCacheEntry {
                addresses,
                expires_at: Instant::now() + DNS_CACHE_TTL,
            },
        );
    }

    #[cfg(test)]
    pub(crate) fn seed(&self, host: &str, addresses: Vec<IpAddr>) {
        self.store(host, Arc::from(addresses.into_boxed_slice()));
    }
}

#[cfg(test)]
mod tests {
    use std::net::{IpAddr, Ipv4Addr};

    use super::shared_dns_cache;

    #[tokio::test]
    async fn ip_literals_bypass_the_cache_and_hostnames_use_seeded_entries() {
        let cache = shared_dns_cache();
        let literal = cache
            .resolve("192.0.2.7")
            .await
            .expect("IP literal resolution");
        assert_eq!(literal.as_ref(), [IpAddr::V4(Ipv4Addr::new(192, 0, 2, 7))]);

        let seeded = vec![
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 1)),
            IpAddr::V4(Ipv4Addr::new(192, 0, 2, 2)),
        ];
        cache.seed("seeded.invalid", seeded.clone());
        let resolved = cache
            .resolve("seeded.invalid")
            .await
            .expect("seeded resolution never hits real DNS");
        assert_eq!(resolved.as_ref(), seeded);
    }
}
