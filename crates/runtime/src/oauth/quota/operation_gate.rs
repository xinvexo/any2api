use std::{
    collections::HashMap,
    sync::{Arc, Mutex, Weak, atomic::AtomicU64},
};

use any2api_domain::OAuthAccountId;
use tokio::sync::Mutex as AsyncMutex;

use super::types::{OAuthQuotaError, OAuthQuotaSnapshot};

#[derive(Default)]
pub(super) struct OAuthQuotaOperationGates {
    gates: Mutex<HashMap<OAuthAccountId, Weak<OAuthQuotaOperationGate>>>,
}

impl OAuthQuotaOperationGates {
    pub(super) fn get(&self, id: OAuthAccountId) -> Arc<OAuthQuotaOperationGate> {
        let mut gates = self.gates.lock().expect("OAuth quota gate lock poisoned");
        gates.retain(|_, gate| gate.strong_count() > 0);
        if let Some(gate) = gates.get(&id).and_then(Weak::upgrade) {
            return gate;
        }
        let gate = Arc::new(OAuthQuotaOperationGate::default());
        gates.insert(id, Arc::downgrade(&gate));
        gate
    }
}

#[derive(Default)]
pub(super) struct OAuthQuotaOperationGate {
    pub(super) operation_completed: AtomicU64,
    pub(super) state: AsyncMutex<OAuthQuotaOperationState>,
}

#[derive(Default)]
pub(super) struct OAuthQuotaOperationState {
    pub(super) last_completed: Option<OAuthQuotaCompletedOperation>,
}

pub(super) enum OAuthQuotaCompletedOperation {
    Refresh(Arc<Result<OAuthQuotaSnapshot, OAuthQuotaError>>),
    Reset,
}
