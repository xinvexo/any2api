#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LogPage<T> {
    pub items: Vec<T>,
    pub total: u64,
}

impl<T> LogPage<T> {
    #[must_use]
    pub const fn new(items: Vec<T>, total: u64) -> Self {
        Self { items, total }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self::new(Vec::new(), 0)
    }
}
