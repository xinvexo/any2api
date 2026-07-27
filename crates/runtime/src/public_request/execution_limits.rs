use std::time::Duration;

use any2api_domain::ProtocolOperation;

pub const STANDARD_PUBLIC_REQUEST_BODY_LIMIT_BYTES: usize = 32 * 1024 * 1024;
pub const IMAGES_EDIT_REQUEST_BODY_LIMIT_BYTES: usize = 512 * 1024 * 1024;

pub(super) const STANDARD_BUFFERED_RESPONSE_LIMIT_BYTES: usize = 16 * 1024 * 1024;
const IMAGES_BUFFERED_RESPONSE_LIMIT_BYTES: usize = 512 * 1024 * 1024;
const IMAGES_SSE_PRECOMMIT_LIMIT_BYTES: usize = 128 * 1024 * 1024;
const IMAGES_MINIMUM_TIMEOUT: Duration = Duration::from_secs(180);

pub(super) const fn is_images(operation: ProtocolOperation) -> bool {
    matches!(
        operation,
        ProtocolOperation::ImagesGenerations | ProtocolOperation::ImagesEdits
    )
}

pub(super) const fn buffered_response_limit(
    operation: ProtocolOperation,
    successful: bool,
) -> usize {
    if successful && is_images(operation) {
        IMAGES_BUFFERED_RESPONSE_LIMIT_BYTES
    } else {
        STANDARD_BUFFERED_RESPONSE_LIMIT_BYTES
    }
}

pub(super) fn read_timeout(operation: ProtocolOperation, configured: Duration) -> Duration {
    image_timeout_floor(operation, configured)
}

pub(super) fn retry_budget(operation: ProtocolOperation, configured: Duration) -> Duration {
    image_timeout_floor(operation, configured)
}

pub(super) fn stream_precommit_bytes(operation: ProtocolOperation, configured: usize) -> usize {
    if is_images(operation) {
        IMAGES_SSE_PRECOMMIT_LIMIT_BYTES
    } else {
        configured
    }
}

pub(super) fn stream_timeout(operation: ProtocolOperation, configured: Duration) -> Duration {
    image_timeout_floor(operation, configured)
}

fn image_timeout_floor(operation: ProtocolOperation, configured: Duration) -> Duration {
    if is_images(operation) {
        configured.max(IMAGES_MINIMUM_TIMEOUT)
    } else {
        configured
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use any2api_domain::ProtocolOperation;

    use super::{
        IMAGES_BUFFERED_RESPONSE_LIMIT_BYTES, IMAGES_EDIT_REQUEST_BODY_LIMIT_BYTES,
        IMAGES_MINIMUM_TIMEOUT, IMAGES_SSE_PRECOMMIT_LIMIT_BYTES,
        STANDARD_BUFFERED_RESPONSE_LIMIT_BYTES, buffered_response_limit, read_timeout,
        retry_budget, stream_precommit_bytes, stream_timeout,
    };

    #[test]
    fn images_use_dedicated_buffer_limits_without_widening_text() {
        assert_eq!(
            buffered_response_limit(ProtocolOperation::ImagesGenerations, true),
            IMAGES_BUFFERED_RESPONSE_LIMIT_BYTES
        );
        assert_eq!(
            buffered_response_limit(ProtocolOperation::ImagesEdits, true),
            IMAGES_BUFFERED_RESPONSE_LIMIT_BYTES
        );
        assert_eq!(
            stream_precommit_bytes(ProtocolOperation::ImagesGenerations, 256 * 1024),
            IMAGES_SSE_PRECOMMIT_LIMIT_BYTES
        );
        assert_eq!(IMAGES_EDIT_REQUEST_BODY_LIMIT_BYTES, 512 * 1024 * 1024);

        assert_eq!(
            buffered_response_limit(ProtocolOperation::Responses, true),
            STANDARD_BUFFERED_RESPONSE_LIMIT_BYTES
        );
        assert_eq!(
            buffered_response_limit(ProtocolOperation::ImagesGenerations, false),
            STANDARD_BUFFERED_RESPONSE_LIMIT_BYTES
        );
        assert_eq!(
            stream_precommit_bytes(ProtocolOperation::Responses, 256 * 1024),
            256 * 1024
        );
    }

    #[test]
    fn images_apply_a_timeout_floor_without_shortening_larger_settings() {
        let short = Duration::from_secs(15);
        let long = Duration::from_secs(240);

        for operation in [
            ProtocolOperation::ImagesGenerations,
            ProtocolOperation::ImagesEdits,
        ] {
            assert_eq!(read_timeout(operation, short), IMAGES_MINIMUM_TIMEOUT);
            assert_eq!(retry_budget(operation, short), IMAGES_MINIMUM_TIMEOUT);
            assert_eq!(stream_timeout(operation, short), IMAGES_MINIMUM_TIMEOUT);
            assert_eq!(read_timeout(operation, long), long);
        }

        assert_eq!(read_timeout(ProtocolOperation::Responses, short), short);
        assert_eq!(retry_budget(ProtocolOperation::Responses, short), short);
        assert_eq!(stream_timeout(ProtocolOperation::Responses, short), short);
    }
}
