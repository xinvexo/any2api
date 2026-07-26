use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};

use any2api_domain::{
    ConfigRevision, HttpAccessLog, HttpAccessLogOutcome, HttpProtocolVersion, LoggingSettings,
    RequestId,
};
use any2api_runtime::api::RequestTelemetry;
use axum::body::{Body, Bytes, HttpBody};
use http_body::{Frame, SizeHint};

pub(super) struct AccessLogCompletion {
    telemetry: Arc<RequestTelemetry>,
    settings: LoggingSettings,
    metadata: AccessLogMetadata,
    started: Instant,
    status_code: Option<u16>,
    pending: bool,
}

pub(super) struct AccessLogMetadata {
    request_id: RequestId,
    started_at_ms: u64,
    config_revision: ConfigRevision,
    client_ip: Option<std::net::IpAddr>,
    method: String,
    path: String,
    http_version: HttpProtocolVersion,
}

impl AccessLogMetadata {
    pub(super) fn new(
        request_id: RequestId,
        started_at_ms: u64,
        config_revision: ConfigRevision,
        client_ip: Option<std::net::IpAddr>,
        method: String,
        path: String,
        http_version: HttpProtocolVersion,
    ) -> Self {
        Self {
            request_id,
            started_at_ms,
            config_revision,
            client_ip,
            method,
            path,
            http_version,
        }
    }
}

impl AccessLogCompletion {
    pub(super) fn new(
        telemetry: Arc<RequestTelemetry>,
        settings: LoggingSettings,
        metadata: AccessLogMetadata,
        started: Instant,
    ) -> Self {
        Self {
            telemetry,
            settings,
            metadata,
            started,
            status_code: None,
            pending: true,
        }
    }

    pub(super) fn set_status(&mut self, status_code: u16) {
        self.status_code = Some(status_code);
    }

    pub(super) fn exclude(&mut self) {
        self.pending = false;
    }

    fn finish(&mut self, outcome: HttpAccessLogOutcome, response_bytes: u64) {
        if !self.pending {
            return;
        }
        self.pending = false;
        let duration_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        self.telemetry.try_record_http_access(
            HttpAccessLog {
                request_id: self.metadata.request_id,
                started_at_ms: self.metadata.started_at_ms,
                config_revision: self.metadata.config_revision,
                client_ip: self.metadata.client_ip,
                method: self.metadata.method.clone(),
                path: self.metadata.path.clone(),
                http_version: self.metadata.http_version,
                status_code: self.status_code,
                duration_ms,
                response_bytes,
                outcome,
            },
            &self.settings,
        );
    }
}

impl Drop for AccessLogCompletion {
    fn drop(&mut self) {
        self.finish(HttpAccessLogOutcome::Cancelled, 0);
    }
}

pub(super) struct AccessLogBody {
    inner: Body,
    completion: Option<AccessLogCompletion>,
    response_bytes: u64,
}

impl AccessLogBody {
    pub(super) fn new(inner: Body, completion: AccessLogCompletion) -> Self {
        Self {
            inner,
            completion: Some(completion),
            response_bytes: 0,
        }
    }

    fn finish(&mut self, outcome: HttpAccessLogOutcome) {
        if let Some(mut completion) = self.completion.take() {
            completion.finish(outcome, self.response_bytes);
        }
    }
}

impl HttpBody for AccessLogBody {
    type Data = Bytes;
    type Error = axum::Error;

    fn poll_frame(
        mut self: Pin<&mut Self>,
        context: &mut Context<'_>,
    ) -> Poll<Option<Result<Frame<Self::Data>, Self::Error>>> {
        let frame = Pin::new(&mut self.inner).poll_frame(context);
        match &frame {
            Poll::Ready(Some(Ok(frame))) => {
                if let Some(data) = frame.data_ref() {
                    self.response_bytes = self
                        .response_bytes
                        .saturating_add(u64::try_from(data.len()).unwrap_or(u64::MAX));
                }
                if self.inner.is_end_stream() {
                    self.finish(HttpAccessLogOutcome::Completed);
                }
            }
            Poll::Ready(Some(Err(_))) => self.finish(HttpAccessLogOutcome::BodyError),
            Poll::Ready(None) => self.finish(HttpAccessLogOutcome::Completed),
            Poll::Pending => {}
        }
        frame
    }

    fn is_end_stream(&self) -> bool {
        self.inner.is_end_stream()
    }

    fn size_hint(&self) -> SizeHint {
        self.inner.size_hint()
    }
}

impl Drop for AccessLogBody {
    fn drop(&mut self) {
        let outcome = if self.inner.is_end_stream() {
            HttpAccessLogOutcome::Completed
        } else {
            HttpAccessLogOutcome::Cancelled
        };
        self.finish(outcome);
    }
}
