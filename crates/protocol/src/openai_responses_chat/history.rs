use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

use serde_json::Value;

const MAX_HISTORY_ENTRIES: usize = 1_024;
/// Entries are whole conversations, so a count cap alone still admits
/// hundreds of megabytes; bound the estimated total bytes as well.
const MAX_HISTORY_BYTES: usize = 64 * 1024 * 1024;
const HISTORY_TTL: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Default)]
pub(super) struct ChatHistoryStore {
    inner: Mutex<HistoryState>,
}

#[derive(Default)]
struct HistoryState {
    entries: HashMap<String, HistoryEntry>,
    order: VecDeque<String>,
    total_bytes: usize,
}

struct HistoryEntry {
    messages: Vec<Value>,
    estimated_bytes: usize,
    expires_at: Instant,
}

impl ChatHistoryStore {
    pub(super) fn get(&self, response_id: &str) -> Option<Vec<Value>> {
        let mut state = self.inner.lock().ok()?;
        state.prune();
        state
            .entries
            .get(response_id)
            .map(|entry| entry.messages.clone())
    }

    pub(super) fn insert(&self, response_id: String, messages: Vec<Value>) {
        let Ok(mut state) = self.inner.lock() else {
            return;
        };
        state.prune();
        let estimated_bytes = messages.iter().map(estimated_json_bytes).sum();
        state.remove(&response_id);
        state.order.push_back(response_id.clone());
        state.total_bytes += estimated_bytes;
        state.entries.insert(
            response_id,
            HistoryEntry {
                messages,
                estimated_bytes,
                expires_at: Instant::now() + HISTORY_TTL,
            },
        );
        while state.entries.len() > MAX_HISTORY_ENTRIES
            || (state.total_bytes > MAX_HISTORY_BYTES && state.entries.len() > 1)
        {
            let Some(oldest) = state.order.pop_front() else {
                break;
            };
            if let Some(entry) = state.entries.remove(&oldest) {
                state.total_bytes = state.total_bytes.saturating_sub(entry.estimated_bytes);
            }
        }
    }
}

/// Serialized size without materializing the JSON string.
fn estimated_json_bytes(message: &Value) -> usize {
    struct CountingWriter(usize);
    impl std::io::Write for CountingWriter {
        fn write(&mut self, buffer: &[u8]) -> std::io::Result<usize> {
            self.0 += buffer.len();
            Ok(buffer.len())
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let mut writer = CountingWriter(0);
    match serde_json::to_writer(&mut writer, message) {
        Ok(()) => writer.0,
        Err(_) => 0,
    }
}

impl HistoryState {
    fn prune(&mut self) {
        let now = Instant::now();
        let mut freed = 0usize;
        self.entries.retain(|_, entry| {
            let keep = entry.expires_at > now;
            if !keep {
                freed += entry.estimated_bytes;
            }
            keep
        });
        self.total_bytes = self.total_bytes.saturating_sub(freed);
        self.order.retain(|id| self.entries.contains_key(id));
    }

    fn remove(&mut self, response_id: &str) {
        if let Some(entry) = self.entries.remove(response_id) {
            self.total_bytes = self.total_bytes.saturating_sub(entry.estimated_bytes);
        }
        self.order.retain(|id| id != response_id);
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::ChatHistoryStore;

    #[test]
    fn stores_and_replaces_completed_conversations() {
        let history = ChatHistoryStore::default();
        history.insert(
            "resp_1".into(),
            vec![json!({"role":"user","content":"one"})],
        );
        history.insert(
            "resp_1".into(),
            vec![json!({"role":"user","content":"two"})],
        );

        assert_eq!(
            history.get("resp_1"),
            Some(vec![json!({"role":"user","content":"two"})])
        );
    }
}
