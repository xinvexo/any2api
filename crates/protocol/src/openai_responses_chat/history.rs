use std::{
    collections::{HashMap, VecDeque},
    sync::Mutex,
    time::{Duration, Instant},
};

use serde_json::Value;

const MAX_HISTORY_ENTRIES: usize = 1_024;
const HISTORY_TTL: Duration = Duration::from_secs(2 * 60 * 60);

#[derive(Default)]
pub(super) struct ChatHistoryStore {
    inner: Mutex<HistoryState>,
}

#[derive(Default)]
struct HistoryState {
    entries: HashMap<String, HistoryEntry>,
    order: VecDeque<String>,
}

struct HistoryEntry {
    messages: Vec<Value>,
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
        state.order.retain(|id| id != &response_id);
        state.order.push_back(response_id.clone());
        state.entries.insert(
            response_id,
            HistoryEntry {
                messages,
                expires_at: Instant::now() + HISTORY_TTL,
            },
        );
        while state.entries.len() > MAX_HISTORY_ENTRIES {
            let Some(oldest) = state.order.pop_front() else {
                break;
            };
            state.entries.remove(&oldest);
        }
    }
}

impl HistoryState {
    fn prune(&mut self) {
        let now = Instant::now();
        self.entries.retain(|_, entry| entry.expires_at > now);
        self.order.retain(|id| self.entries.contains_key(id));
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
