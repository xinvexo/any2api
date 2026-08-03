mod capacity;
mod repository;
mod rows;
mod writes;

#[cfg(test)]
mod tests;

#[cfg(test)]
pub(crate) use repository::SYSTEM_LOG_RETENTION_PREDICATE;
pub use repository::{HttpAccessLogCapacity, HttpAccessLogRepository};
