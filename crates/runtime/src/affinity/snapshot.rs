#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AffinityRuntimeSnapshot {
    pub(crate) active_session_count: usize,
    pub(crate) creating_session_count: usize,
}

impl AffinityRuntimeSnapshot {
    pub const fn active_session_count(&self) -> usize {
        self.active_session_count
    }

    pub const fn creating_session_count(&self) -> usize {
        self.creating_session_count
    }
}
