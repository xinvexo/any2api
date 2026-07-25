use std::{
    io,
    pin::Pin,
    task::{Context, Poll},
};

use bytes::Bytes;
use http_body::{Body, Frame, SizeHint};
use tokio::sync::oneshot;

pub(super) fn signaled_request_body(bytes: Bytes) -> (reqwest::Body, oneshot::Receiver<()>) {
    let (body, receiver) = signaled_body(bytes);
    (reqwest::Body::wrap(body), receiver)
}

pub(super) fn signaled_body(bytes: Bytes) -> (SignaledBody, oneshot::Receiver<()>) {
    let (sender, receiver) = oneshot::channel();
    let body = SignaledBody {
        bytes: Some(bytes),
        sender: Some(sender),
    };
    (body, receiver)
}

pub(super) struct SignaledBody {
    bytes: Option<Bytes>,
    sender: Option<oneshot::Sender<()>>,
}

impl Body for SignaledBody {
    type Data = Bytes;
    type Error = io::Error;

    fn poll_frame(
        self: Pin<&mut Self>,
        _context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let this = self.get_mut();
        if let Some(sender) = this.sender.take() {
            let _ = sender.send(());
        }
        if let Some(bytes) = this.bytes.take()
            && !bytes.is_empty()
        {
            return Poll::Ready(Some(Ok(Frame::data(bytes))));
        }
        Poll::Ready(None)
    }

    fn is_end_stream(&self) -> bool {
        self.bytes.is_none()
    }

    fn size_hint(&self) -> SizeHint {
        let mut hint = SizeHint::new();
        hint.set_exact(self.bytes.as_ref().map_or(0, |bytes| bytes.len()) as u64);
        hint
    }
}
