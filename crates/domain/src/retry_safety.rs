use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum RetrySafety {
    DefinitelyNotSent,
    RejectedBeforeExecution,
    Idempotent,
    Ambiguous,
}

impl RetrySafety {
    #[must_use]
    pub const fn allows_automatic_retry(self) -> bool {
        !matches!(self, Self::Ambiguous)
    }
}
