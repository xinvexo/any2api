use any2api_payload_buffer::{PayloadBuffer, PayloadBufferError};
use axum::body::{Body, Bytes, HttpBody};
use futures_util::StreamExt;

pub(super) async fn collect_body(
    body: Body,
    max_bytes: usize,
) -> Result<Bytes, BodyCollectionError> {
    let expected_len = body
        .size_hint()
        .exact()
        .and_then(|len| usize::try_from(len).ok());
    let mut stream = body.into_data_stream();
    let Some(first) = stream.next().await else {
        return Ok(Bytes::new());
    };
    let first = first.map_err(|_| BodyCollectionError::Unreadable)?;
    check_len(0, first.len(), max_bytes)?;
    let Some(second) = stream.next().await else {
        return Ok(first);
    };
    let second = second.map_err(|_| BodyCollectionError::Unreadable)?;
    let mut collected =
        PayloadBuffer::with_capacity_hint(expected_len, max_bytes).map_err(map_buffer_error)?;
    collected
        .extend_from_slice(&first)
        .map_err(map_buffer_error)?;
    collected
        .extend_from_slice(&second)
        .map_err(map_buffer_error)?;

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| BodyCollectionError::Unreadable)?;
        collected
            .extend_from_slice(&chunk)
            .map_err(map_buffer_error)?;
    }

    Ok(collected.freeze().into_bytes())
}

fn map_buffer_error(error: PayloadBufferError) -> BodyCollectionError {
    match error {
        PayloadBufferError::TooLarge => BodyCollectionError::TooLarge,
        PayloadBufferError::AllocationFailed => BodyCollectionError::AllocationFailed,
    }
}

fn check_len(current: usize, added: usize, max_bytes: usize) -> Result<usize, BodyCollectionError> {
    let next = current
        .checked_add(added)
        .ok_or(BodyCollectionError::TooLarge)?;
    if next > max_bytes {
        Err(BodyCollectionError::TooLarge)
    } else {
        Ok(next)
    }
}

#[derive(Debug, Eq, PartialEq)]
pub(super) enum BodyCollectionError {
    TooLarge,
    Unreadable,
    AllocationFailed,
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use axum::body::{Body, Bytes};
    use futures_util::stream;

    use any2api_payload_buffer::PayloadBufferError;

    use super::{BodyCollectionError, collect_body, map_buffer_error};

    #[tokio::test]
    async fn single_chunk_reuses_the_original_bytes() {
        let original = Bytes::from_static(b"single chunk");
        let collected = collect_body(Body::from(original.clone()), 64)
            .await
            .expect("body");

        assert_eq!(collected, original);
        assert_eq!(collected.as_ptr(), original.as_ptr());
    }

    #[tokio::test]
    async fn multiple_chunks_are_joined_and_checked_by_actual_length() {
        let chunks = stream::iter([
            Ok::<_, Infallible>(Bytes::from_static(b"one")),
            Ok(Bytes::from_static(b"two")),
            Ok(Bytes::from_static(b"three")),
        ]);
        let collected = collect_body(Body::from_stream(chunks), 11)
            .await
            .expect("body");
        assert_eq!(collected, Bytes::from_static(b"onetwothree"));

        let oversized = stream::iter([
            Ok::<_, Infallible>(Bytes::from_static(b"one")),
            Ok(Bytes::from_static(b"two")),
        ]);
        assert_eq!(
            collect_body(Body::from_stream(oversized), 5).await,
            Err(BodyCollectionError::TooLarge)
        );
    }

    #[test]
    fn allocation_failure_is_not_reported_as_client_input_failure() {
        assert_eq!(
            map_buffer_error(PayloadBufferError::AllocationFailed),
            BodyCollectionError::AllocationFailed
        );
    }
}
