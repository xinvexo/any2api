use tokio::time::Instant;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum HealthAcquireError {
    Temporary(Instant),
    Permanent,
}
