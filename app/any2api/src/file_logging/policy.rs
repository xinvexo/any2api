use std::sync::RwLock;

use any2api_domain::{ConfigRevision, LoggingSettings};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(super) struct FileLogPolicy {
    pub(super) revision: ConfigRevision,
    pub(super) retention_secs: u64,
    pub(super) max_total_size: u64,
}

impl FileLogPolicy {
    pub(super) fn from_settings(revision: ConfigRevision, settings: &LoggingSettings) -> Self {
        Self {
            revision,
            retention_secs: settings.file_retention_secs(),
            max_total_size: settings.file_max_total_size(),
        }
    }
}

pub(super) fn update_policy(lock: &RwLock<FileLogPolicy>, next: FileLogPolicy) -> bool {
    let mut current = lock.write().expect("file log policy");
    if next.revision < current.revision {
        return false;
    }
    *current = next;
    true
}
