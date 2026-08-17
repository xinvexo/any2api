#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogCursorPosition {
    started_at_ms: u64,
    request_id: String,
}

impl LogCursorPosition {
    #[must_use]
    pub fn new(started_at_ms: u64, request_id: String) -> Self {
        Self {
            started_at_ms,
            request_id,
        }
    }

    #[must_use]
    pub const fn started_at_ms(&self) -> u64 {
        self.started_at_ms
    }

    #[must_use]
    pub fn request_id(&self) -> &str {
        &self.request_id
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogCursor {
    anchor: LogCursorPosition,
    before: Option<LogCursorPosition>,
}

impl LogCursor {
    #[must_use]
    pub const fn first(anchor: LogCursorPosition) -> Self {
        Self {
            anchor,
            before: None,
        }
    }

    #[must_use]
    pub fn next(anchor: LogCursorPosition, before: LogCursorPosition) -> Option<Self> {
        (before <= anchor).then_some(Self {
            anchor,
            before: Some(before),
        })
    }

    #[must_use]
    pub const fn anchor(&self) -> &LogCursorPosition {
        &self.anchor
    }

    #[must_use]
    pub const fn before(&self) -> Option<&LogCursorPosition> {
        self.before.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogBatch<T> {
    pub items: Vec<T>,
    pub next_cursor: Option<LogCursor>,
}

impl<T> LogBatch<T> {
    #[must_use]
    pub const fn new(items: Vec<T>, next_cursor: Option<LogCursor>) -> Self {
        Self { items, next_cursor }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self::new(Vec::new(), None)
    }
}
