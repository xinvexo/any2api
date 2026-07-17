use std::{pin::Pin, result::Result as StdResult};

use async_trait::async_trait;
use bytes::Bytes;
use futures_core::Stream;
use http::{HeaderMap, Method, StatusCode, Uri};

pub use crate::{TransportError, TransportErrorStage};

pub type BoxByteStream =
    Pin<Box<dyn Stream<Item = StdResult<Bytes, TransportError>> + Send + 'static>>;

#[derive(Clone, Debug)]
pub struct TransportRequest {
    pub method: Method,
    pub uri: Uri,
    pub headers: HeaderMap,
    pub body: Bytes,
}

pub struct TransportResponse {
    pub status: StatusCode,
    pub headers: HeaderMap,
    pub body: BoxByteStream,
}

#[async_trait]
pub trait TransportManager: Send + Sync {
    async fn execute(&self, request: TransportRequest)
    -> Result<TransportResponse, TransportError>;
}
