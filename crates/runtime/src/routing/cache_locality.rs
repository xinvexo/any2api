use std::{
    fmt,
    sync::{Arc, Mutex},
    time::Duration,
};

use any2api_domain::{ModelRouteId, ProtocolDialect, ProtocolOperation};
use hashlink::LinkedHashMap;
use hmac::{Hmac, Mac};
use sha2::Sha256;
use tokio::time::Instant;

use super::{CandidateIdentity, RouteCandidate};

const CACHE_LOCALITY_CAPACITY: usize = 16_384;
const CACHE_LOCALITY_TTL: Duration = Duration::from_secs(30 * 60);
type HmacSha256 = Hmac<Sha256>;

#[derive(Clone, Copy, Eq, Hash, PartialEq)]
pub(crate) struct CacheLocalityKey([u8; 32]);

impl fmt::Debug for CacheLocalityKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("CacheLocalityKey([REDACTED])")
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct CacheLocalityTarget {
    identity: CandidateIdentity,
    upstream_model: String,
    upstream_protocol_dialect: ProtocolDialect,
}

impl CacheLocalityTarget {
    pub(crate) fn from_candidate(candidate: &RouteCandidate) -> Self {
        Self {
            identity: candidate.identity(),
            upstream_model: candidate.upstream_model.clone(),
            upstream_protocol_dialect: candidate.upstream_protocol_dialect,
        }
    }

    pub(crate) fn matches_candidate(&self, candidate: &RouteCandidate) -> bool {
        self.identity == candidate.identity()
            && self.upstream_model == candidate.upstream_model
            && self.upstream_protocol_dialect == candidate.upstream_protocol_dialect
    }
}

struct CacheLocalityEntry {
    target: CacheLocalityTarget,
    expires_at: Instant,
}

#[derive(Default)]
struct CacheLocalityEntries {
    values: LinkedHashMap<CacheLocalityKey, CacheLocalityEntry>,
}

pub(crate) struct CacheLocalityRegistry {
    hmac_key: [u8; 32],
    capacity: usize,
    entries: Mutex<CacheLocalityEntries>,
}

impl CacheLocalityRegistry {
    pub(crate) fn new() -> Arc<Self> {
        let mut hmac_key = [0_u8; 32];
        getrandom::fill(&mut hmac_key).expect("operating system randomness is required");
        Arc::new(Self {
            hmac_key,
            capacity: CACHE_LOCALITY_CAPACITY,
            entries: Mutex::new(CacheLocalityEntries::default()),
        })
    }

    pub(crate) fn key(
        &self,
        dialect: ProtocolDialect,
        operation: ProtocolOperation,
        route_id: ModelRouteId,
        raw: &str,
    ) -> CacheLocalityKey {
        let mut mac = HmacSha256::new_from_slice(&self.hmac_key).expect("HMAC accepts 256-bit key");
        mac.update(b"any2api-prompt-cache-locality-v1\0");
        mac.update(&[dialect_code(dialect), operation_code(operation)]);
        mac.update(route_id.as_uuid().as_bytes());
        mac.update(&[0]);
        mac.update(raw.as_bytes());
        CacheLocalityKey(mac.finalize().into_bytes().into())
    }

    pub(crate) fn lookup(&self, key: CacheLocalityKey) -> Option<CacheLocalityTarget> {
        let now = Instant::now();
        let mut entries = self.entries.lock().expect("cache locality lock poisoned");
        if entries
            .values
            .get(&key)
            .is_some_and(|entry| entry.expires_at <= now)
        {
            entries.values.remove(&key);
            return None;
        }
        let entry = entries.values.to_back(&key)?;
        entry.expires_at = now + CACHE_LOCALITY_TTL;
        Some(entry.target.clone())
    }

    pub(crate) fn remember_candidate(&self, key: CacheLocalityKey, candidate: &RouteCandidate) {
        self.remember_target(key, CacheLocalityTarget::from_candidate(candidate));
    }

    pub(crate) fn completion(
        self: &Arc<Self>,
        key: CacheLocalityKey,
        candidate: &RouteCandidate,
    ) -> CacheLocalityCompletion {
        CacheLocalityCompletion {
            registry: Arc::clone(self),
            key,
            target: CacheLocalityTarget::from_candidate(candidate),
        }
    }

    pub(crate) fn forget_candidate(&self, key: CacheLocalityKey, candidate: &RouteCandidate) {
        let mut entries = self.entries.lock().expect("cache locality lock poisoned");
        if entries
            .values
            .get(&key)
            .is_some_and(|entry| entry.target.matches_candidate(candidate))
        {
            entries.values.remove(&key);
        }
    }

    pub(crate) fn forget_target(&self, key: CacheLocalityKey, target: &CacheLocalityTarget) {
        let mut entries = self.entries.lock().expect("cache locality lock poisoned");
        if entries
            .values
            .get(&key)
            .is_some_and(|entry| entry.target == *target)
        {
            entries.values.remove(&key);
        }
    }

    fn remember_target(&self, key: CacheLocalityKey, target: CacheLocalityTarget) {
        if self.capacity == 0 {
            return;
        }
        let now = Instant::now();
        let mut entries = self.entries.lock().expect("cache locality lock poisoned");
        if !entries.values.contains_key(&key) && entries.values.len() >= self.capacity {
            entries.values.retain(|_, entry| entry.expires_at > now);
            if entries.values.len() >= self.capacity {
                entries.values.pop_front();
            }
        }
        entries.values.insert(
            key,
            CacheLocalityEntry {
                target,
                expires_at: now + CACHE_LOCALITY_TTL,
            },
        );
    }

    #[cfg(test)]
    fn with_capacity_for_test(capacity: usize) -> Arc<Self> {
        let mut registry = Self::new();
        Arc::get_mut(&mut registry)
            .expect("new registry is uniquely owned")
            .capacity = capacity;
        registry
    }

    #[cfg(test)]
    fn len_for_test(&self) -> usize {
        self.entries
            .lock()
            .expect("cache locality lock poisoned")
            .values
            .len()
    }
}

impl fmt::Debug for CacheLocalityRegistry {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("CacheLocalityRegistry")
            .field("capacity", &self.capacity)
            .finish_non_exhaustive()
    }
}

pub(crate) struct CacheLocalityCompletion {
    registry: Arc<CacheLocalityRegistry>,
    key: CacheLocalityKey,
    target: CacheLocalityTarget,
}

impl CacheLocalityCompletion {
    pub(crate) fn success(self) {
        self.registry.remember_target(self.key, self.target);
    }

    pub(crate) fn failure(self) {
        self.registry.forget_target(self.key, &self.target);
    }
}

const fn dialect_code(dialect: ProtocolDialect) -> u8 {
    match dialect {
        ProtocolDialect::OpenAiResponses => 1,
        ProtocolDialect::OpenAiChatCompletions => 2,
        ProtocolDialect::OpenAiImages => 3,
        ProtocolDialect::AnthropicMessages => 4,
    }
}

const fn operation_code(operation: ProtocolOperation) -> u8 {
    match operation {
        ProtocolOperation::Responses => 1,
        ProtocolOperation::ResponsesCompact => 2,
        ProtocolOperation::ChatCompletions => 3,
        ProtocolOperation::ImagesGenerations => 4,
        ProtocolOperation::ImagesEdits => 5,
        ProtocolOperation::Messages => 6,
        ProtocolOperation::MessagesCountTokens => 7,
    }
}

#[cfg(test)]
mod tests;
