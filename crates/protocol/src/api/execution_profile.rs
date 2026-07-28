#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum RequestExecutionProfile {
    #[default]
    Standard,
    RemoteCompaction,
}

impl RequestExecutionProfile {
    #[must_use]
    pub const fn requires_same_dialect(self) -> bool {
        matches!(self, Self::RemoteCompaction)
    }
}
