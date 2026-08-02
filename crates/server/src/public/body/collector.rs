use axum::body::{Body, Bytes};
use futures_util::StreamExt;

pub(super) async fn collect_body(
    body: Body,
    max_bytes: usize,
) -> Result<Bytes, BodyCollectionError> {
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
    let initial_len = check_len(first.len(), second.len(), max_bytes)?;
    let mut collected = Vec::with_capacity(initial_len);
    collected.extend_from_slice(&first);
    collected.extend_from_slice(&second);

    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|_| BodyCollectionError::Unreadable)?;
        check_len(collected.len(), chunk.len(), max_bytes)?;
        collected.extend_from_slice(&chunk);
    }

    Ok(Bytes::from(collected))
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
}

#[cfg(test)]
mod tests {
    use std::convert::Infallible;

    use axum::body::{Body, Bytes};
    use futures_util::stream;

    use super::{BodyCollectionError, collect_body};

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
}
