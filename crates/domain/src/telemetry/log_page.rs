#[derive(Clone, Debug, Eq, Ord, PartialEq, PartialOrd)]
pub struct LogPagePosition {
    started_at_ms: u64,
    request_id: String,
}

impl LogPagePosition {
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
pub struct LogPageCursor {
    anchor: LogPagePosition,
    before: Option<LogPagePosition>,
}

impl LogPageCursor {
    #[must_use]
    pub const fn first(anchor: LogPagePosition) -> Self {
        Self {
            anchor,
            before: None,
        }
    }

    #[must_use]
    pub fn next(anchor: LogPagePosition, before: LogPagePosition) -> Option<Self> {
        (before <= anchor).then_some(Self {
            anchor,
            before: Some(before),
        })
    }

    #[must_use]
    pub const fn anchor(&self) -> &LogPagePosition {
        &self.anchor
    }

    #[must_use]
    pub const fn before(&self) -> Option<&LogPagePosition> {
        self.before.as_ref()
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogPage<T> {
    pub items: Vec<T>,
    pub total: u64,
    pub page: u32,
    pub cursor: Option<LogPageCursor>,
    pub next_cursor: Option<LogPageCursor>,
}

impl<T> LogPage<T> {
    #[must_use]
    pub const fn new(
        items: Vec<T>,
        total: u64,
        page: u32,
        cursor: Option<LogPageCursor>,
        next_cursor: Option<LogPageCursor>,
    ) -> Self {
        Self {
            items,
            total,
            page,
            cursor,
            next_cursor,
        }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self::new(Vec::new(), 0, 1, None, None)
    }
}
