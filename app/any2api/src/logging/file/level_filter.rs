use std::sync::{
    Arc,
    atomic::{AtomicU8, Ordering},
};

use any2api_domain::FileLogLevel;
use tracing::{Level, Metadata, subscriber::Interest};
use tracing_subscriber::{
    filter::LevelFilter,
    layer::{Context, Filter},
};

#[derive(Clone, Debug)]
pub(in crate::logging) struct FileLevelFilter {
    level: Arc<AtomicU8>,
}

impl FileLevelFilter {
    pub(in crate::logging) fn disabled() -> Self {
        Self {
            level: Arc::new(AtomicU8::new(0)),
        }
    }

    pub(in crate::logging) fn set(&self, level: FileLogLevel) {
        self.store(encode(level));
    }

    pub(in crate::logging) fn disable(&self) {
        self.store(0);
    }

    fn store(&self, encoded: u8) {
        if self.level.swap(encoded, Ordering::AcqRel) != encoded {
            // Cached callsite interest and the global max level are derived
            // from the current level below; refresh them so runtime level
            // changes keep taking effect.
            tracing::callsite::rebuild_interest_cache();
        }
    }

    pub(in crate::logging) fn enabled_level(&self, level: &Level) -> bool {
        verbosity(level) <= self.level.load(Ordering::Acquire)
    }
}

impl<S> Filter<S> for FileLevelFilter {
    fn enabled(&self, metadata: &Metadata<'_>, _: &Context<'_, S>) -> bool {
        self.enabled_level(metadata.level())
    }

    fn callsite_enabled(&self, metadata: &'static Metadata<'static>) -> Interest {
        if self.enabled_level(metadata.level()) {
            Interest::always()
        } else {
            Interest::never()
        }
    }

    fn max_level_hint(&self) -> Option<LevelFilter> {
        Some(max_level(self.level.load(Ordering::Acquire)))
    }
}

const fn max_level(encoded: u8) -> LevelFilter {
    match encoded {
        0 => LevelFilter::OFF,
        1 => LevelFilter::ERROR,
        2 => LevelFilter::WARN,
        3 => LevelFilter::INFO,
        4 => LevelFilter::DEBUG,
        _ => LevelFilter::TRACE,
    }
}

const fn encode(level: FileLogLevel) -> u8 {
    match level {
        FileLogLevel::Error => 1,
        FileLogLevel::Warn => 2,
        FileLogLevel::Info => 3,
        FileLogLevel::Debug => 4,
        FileLogLevel::Trace => 5,
    }
}

fn verbosity(level: &Level) -> u8 {
    match *level {
        Level::ERROR => 1,
        Level::WARN => 2,
        Level::INFO => 3,
        Level::DEBUG => 4,
        Level::TRACE => 5,
    }
}
