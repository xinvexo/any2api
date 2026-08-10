use std::{
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll},
};

use tokio::{
    io::{AsyncRead, AsyncWrite, ReadBuf},
    sync::Notify,
};

use super::frames::contains_server_frame;

#[derive(Clone, Default)]
pub(super) struct WireCapture {
    client_to_server: Arc<Mutex<Vec<u8>>>,
    server_to_client: Arc<Mutex<Vec<u8>>>,
    server_write: Arc<Notify>,
}

impl WireCapture {
    pub(super) fn recording<T>(&self, inner: T) -> RecordingIo<T> {
        RecordingIo {
            inner,
            capture: self.clone(),
        }
    }

    pub(super) fn client_bytes(&self) -> Vec<u8> {
        self.client_to_server
            .lock()
            .expect("HTTP/2 client capture lock")
            .clone()
    }

    pub(super) fn server_bytes(&self) -> Vec<u8> {
        self.server_to_client
            .lock()
            .expect("HTTP/2 server capture lock")
            .clone()
    }

    pub(super) async fn wait_for_server_frame(&self, frame_type: u8) {
        loop {
            let notified = self.server_write.notified();
            if contains_server_frame(&self.server_bytes(), frame_type) {
                return;
            }
            notified.await;
        }
    }
}

pub(super) struct RecordingIo<T> {
    inner: T,
    capture: WireCapture,
}

impl<T: AsyncRead + Unpin> AsyncRead for RecordingIo<T> {
    fn poll_read(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &mut ReadBuf<'_>,
    ) -> Poll<std::io::Result<()>> {
        let before = buffer.filled().len();
        let result = Pin::new(&mut self.inner).poll_read(context, buffer);
        if let Poll::Ready(Ok(())) = &result {
            self.capture
                .client_to_server
                .lock()
                .expect("HTTP/2 client capture lock")
                .extend_from_slice(&buffer.filled()[before..]);
        }
        result
    }
}

impl<T: AsyncWrite + Unpin> AsyncWrite for RecordingIo<T> {
    fn poll_write(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
        buffer: &[u8],
    ) -> Poll<std::io::Result<usize>> {
        let result = Pin::new(&mut self.inner).poll_write(context, buffer);
        if let Poll::Ready(Ok(written)) = &result {
            self.capture
                .server_to_client
                .lock()
                .expect("HTTP/2 server capture lock")
                .extend_from_slice(&buffer[..*written]);
            self.capture.server_write.notify_one();
        }
        result
    }

    fn poll_flush(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_flush(context)
    }

    fn poll_shutdown(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<std::io::Result<()>> {
        Pin::new(&mut self.inner).poll_shutdown(context)
    }
}
