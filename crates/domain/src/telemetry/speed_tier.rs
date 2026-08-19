#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RequestSpeedTier {
    Standard,
    Fast,
}

impl RequestSpeedTier {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Standard => "standard",
            Self::Fast => "fast",
        }
    }

    #[must_use]
    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "standard" => Some(Self::Standard),
            "fast" => Some(Self::Fast),
            _ => None,
        }
    }
}
