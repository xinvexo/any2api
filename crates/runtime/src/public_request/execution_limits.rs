use std::time::Duration;

use any2api_domain::ProtocolOperation;
use any2api_protocol::api::RequestExecutionProfile;

pub const STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES: usize = 32 * 1024 * 1024;

pub(super) const STANDARD_BUFFERED_RESPONSE_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const REMOTE_COMPACTION_MINIMUM_TIMEOUT: Duration = Duration::from_secs(300);
const RESPONSES_COMPACT_MINIMUM_TIMEOUT: Duration = Duration::from_secs(1_200);

pub(super) fn read_timeout(
    operation: ProtocolOperation,
    profile: RequestExecutionProfile,
    configured: Duration,
) -> Duration {
    timeout_floor(operation, profile, configured)
}

pub(super) fn retry_budget(
    operation: ProtocolOperation,
    profile: RequestExecutionProfile,
    configured: Duration,
) -> Duration {
    timeout_floor(operation, profile, configured)
}

pub(super) fn stream_timeout(
    operation: ProtocolOperation,
    profile: RequestExecutionProfile,
    configured: Duration,
) -> Duration {
    timeout_floor(operation, profile, configured)
}

fn timeout_floor(
    operation: ProtocolOperation,
    profile: RequestExecutionProfile,
    configured: Duration,
) -> Duration {
    let minimum = match operation {
        ProtocolOperation::ResponsesCompact => RESPONSES_COMPACT_MINIMUM_TIMEOUT,
        _ if profile == RequestExecutionProfile::RemoteCompaction => {
            REMOTE_COMPACTION_MINIMUM_TIMEOUT
        }
        _ => return configured,
    };
    configured.max(minimum)
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use any2api_domain::ProtocolOperation;
    use any2api_protocol::api::RequestExecutionProfile;

    use super::{
        REMOTE_COMPACTION_MINIMUM_TIMEOUT, RESPONSES_COMPACT_MINIMUM_TIMEOUT, read_timeout,
        retry_budget, stream_timeout,
    };

    #[test]
    fn compaction_profiles_apply_protocol_compatible_timeout_floors() {
        let short = Duration::from_secs(1);
        let long = Duration::from_secs(1_500);

        for limit in [read_timeout, retry_budget, stream_timeout] {
            assert_eq!(
                limit(
                    ProtocolOperation::Responses,
                    RequestExecutionProfile::RemoteCompaction,
                    short,
                ),
                REMOTE_COMPACTION_MINIMUM_TIMEOUT
            );
            assert_eq!(
                limit(
                    ProtocolOperation::ResponsesCompact,
                    RequestExecutionProfile::RemoteCompaction,
                    short,
                ),
                RESPONSES_COMPACT_MINIMUM_TIMEOUT
            );
            assert_eq!(
                limit(
                    ProtocolOperation::ResponsesCompact,
                    RequestExecutionProfile::Standard,
                    short,
                ),
                RESPONSES_COMPACT_MINIMUM_TIMEOUT
            );
            assert_eq!(
                limit(
                    ProtocolOperation::Responses,
                    RequestExecutionProfile::RemoteCompaction,
                    long,
                ),
                long
            );
        }
    }
}
