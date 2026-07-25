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

    pub const fn as_str(self) -> &'static str {
        match self {
            Self::DefinitelyNotSent => "definitely_not_sent",
            Self::RejectedBeforeExecution => "rejected_before_execution",
            Self::Idempotent => "idempotent",
            Self::Ambiguous => "ambiguous",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "definitely_not_sent" => Some(Self::DefinitelyNotSent),
            "rejected_before_execution" => Some(Self::RejectedBeforeExecution),
            "idempotent" => Some(Self::Idempotent),
            "ambiguous" => Some(Self::Ambiguous),
            _ => None,
        }
    }
}
