pub const MAX_TOKEN_COUNT: u64 = (1_u64 << 53) - 1;

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TokenUsage {
    input_tokens: Option<u64>,
    output_tokens: Option<u64>,
    cache_read_tokens: Option<u64>,
    cache_write_tokens: Option<u64>,
}

impl TokenUsage {
    #[must_use]
    pub const fn new(
        input_tokens: Option<u64>,
        output_tokens: Option<u64>,
        cache_read_tokens: Option<u64>,
        cache_write_tokens: Option<u64>,
    ) -> Self {
        Self {
            input_tokens: valid_count(input_tokens),
            output_tokens: valid_count(output_tokens),
            cache_read_tokens: valid_count(cache_read_tokens),
            cache_write_tokens: valid_count(cache_write_tokens),
        }
    }

    #[must_use]
    pub const fn input_tokens(self) -> Option<u64> {
        self.input_tokens
    }

    #[must_use]
    pub const fn output_tokens(self) -> Option<u64> {
        self.output_tokens
    }

    #[must_use]
    pub const fn cache_read_tokens(self) -> Option<u64> {
        self.cache_read_tokens
    }

    #[must_use]
    pub const fn cache_write_tokens(self) -> Option<u64> {
        self.cache_write_tokens
    }

    pub fn merge(&mut self, update: Self) {
        if update.input_tokens.is_some() {
            self.input_tokens = update.input_tokens;
        }
        if update.output_tokens.is_some() {
            self.output_tokens = update.output_tokens;
        }
        if update.cache_read_tokens.is_some() {
            self.cache_read_tokens = update.cache_read_tokens;
        }
        if update.cache_write_tokens.is_some() {
            self.cache_write_tokens = update.cache_write_tokens;
        }
    }
}

const fn valid_count(value: Option<u64>) -> Option<u64> {
    match value {
        Some(value) if value <= MAX_TOKEN_COUNT => Some(value),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_TOKEN_COUNT, TokenUsage};

    #[test]
    fn cumulative_updates_replace_only_present_fields() {
        let mut usage = TokenUsage::new(Some(12), Some(1), Some(3), None);

        usage.merge(TokenUsage::new(None, Some(8), None, Some(5)));

        assert_eq!(usage, TokenUsage::new(Some(12), Some(8), Some(3), Some(5)));
    }

    #[test]
    fn counts_must_be_safe_for_sqlite_and_json_consumers() {
        assert_eq!(
            TokenUsage::new(Some(MAX_TOKEN_COUNT), Some(MAX_TOKEN_COUNT + 1), None, None),
            TokenUsage::new(Some(MAX_TOKEN_COUNT), None, None, None)
        );
    }
}
