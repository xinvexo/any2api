use std::time::Duration;

use any2api_domain::RetrySafety;
use any2api_transport::api::{
    BoxByteStream, TransportError, TransportErrorStage, TransportFailureScope,
};
use bytes::{Bytes, BytesMut};
use futures_util::StreamExt;
use tokio::time::timeout;

const MAX_MODEL_CATALOG_BYTES: usize = 1024 * 1024;

pub(crate) async fn collect(
    mut body: BoxByteStream,
    read_timeout: Duration,
    failure_scope: TransportFailureScope,
) -> Result<Bytes, ModelCatalogReadError> {
    let mut collected = BytesMut::new();
    loop {
        let next = timeout(read_timeout, body.next()).await.map_err(|_| {
            ModelCatalogReadError::Transport(TransportError::new(
                TransportErrorStage::ReadBody,
                failure_scope,
                RetrySafety::Ambiguous,
                "provider model catalog read timed out",
            ))
        })?;
        let Some(chunk) = next else {
            break;
        };
        let chunk = chunk.map_err(ModelCatalogReadError::Transport)?;
        if collected.len().saturating_add(chunk.len()) > MAX_MODEL_CATALOG_BYTES {
            return Err(ModelCatalogReadError::TooLarge);
        }
        collected.extend_from_slice(&chunk);
    }
    Ok(collected.freeze())
}

#[derive(Debug)]
pub(crate) enum ModelCatalogReadError {
    Transport(TransportError),
    TooLarge,
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use any2api_domain::RetrySafety;
    use any2api_transport::api::{
        BoxByteStream, TransportError, TransportErrorStage, TransportFailureScope,
    };
    use bytes::Bytes;
    use futures_util::stream;

    use super::{MAX_MODEL_CATALOG_BYTES, ModelCatalogReadError, collect};

    #[tokio::test]
    async fn collects_chunked_catalog_body() {
        let body: BoxByteStream = Box::pin(stream::iter([
            Ok(Bytes::from_static(b"{\"data\":")),
            Ok(Bytes::from_static(b"[]}")),
        ]));

        let collected = collect(
            body,
            Duration::from_secs(1),
            TransportFailureScope::Endpoint,
        )
        .await
        .expect("catalog body");

        assert_eq!(collected, Bytes::from_static(b"{\"data\":[]}"));
    }

    #[tokio::test]
    async fn rejects_oversized_catalog_body() {
        let body: BoxByteStream = Box::pin(stream::iter([Ok(Bytes::from(vec![
            0;
            MAX_MODEL_CATALOG_BYTES
                + 1
        ]))]));

        assert!(matches!(
            collect(
                body,
                Duration::from_secs(1),
                TransportFailureScope::Endpoint,
            )
            .await,
            Err(ModelCatalogReadError::TooLarge)
        ));
    }

    #[tokio::test]
    async fn preserves_transport_read_failures() {
        let body: BoxByteStream = Box::pin(stream::iter([Err(TransportError::new(
            TransportErrorStage::ReadBody,
            TransportFailureScope::Endpoint,
            RetrySafety::Ambiguous,
            "catalog read failed",
        ))]));

        assert!(matches!(
            collect(
                body,
                Duration::from_secs(1),
                TransportFailureScope::Endpoint,
            )
            .await,
            Err(ModelCatalogReadError::Transport(_))
        ));
    }
}
