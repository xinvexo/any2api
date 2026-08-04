use std::{
    pin::Pin,
    sync::Arc,
    task::{Context, Poll},
    time::Instant,
};

use any2api_domain::{
    ConfigRevision, HttpAccessLog, HttpAccessLogExchange, HttpAccessLogOutcome, HttpHeader,
    HttpProtocolVersion, LoggingSettings, RequestId,
};
use any2api_runtime::api::RequestTelemetry;
use axum::body::{Body, Bytes, HttpBody};
use http_body::{Frame, SizeHint};

use super::{
    capture::{BodyCapture, SharedBodyCapture},
    policy::{change_notification, should_record},
};

pub(super) struct AccessLogCompletion {
    telemetry: Arc<RequestTelemetry>,
    settings: LoggingSettings,
    metadata: AccessLogMetadata,
    started: Instant,
    status_code: Option<u16>,
    response_headers: Vec<HttpHeader>,
    gateway_auth_rejected: bool,
    pending: bool,
}

pub(super) struct AccessLogMetadata {
    request_id: RequestId,
    started_at_ms: u64,
    config_revision: ConfigRevision,
    client_ip: Option<std::net::IpAddr>,
    method: String,
    path: String,
    uri: String,
    http_version: HttpProtocolVersion,
    request_headers: Vec<HttpHeader>,
    request_body: SharedBodyCapture,
}

impl AccessLogMetadata {
    #[allow(clippy::too_many_arguments)]
    pub(super) fn new(
        request_id: RequestId,
        started_at_ms: u64,
        config_revision: ConfigRevision,
        client_ip: Option<std::net::IpAddr>,
        method: String,
        path: String,
        uri: String,
        http_version: HttpProtocolVersion,
        request_headers: Vec<HttpHeader>,
        request_body: SharedBodyCapture,
    ) -> Self {
        Self {
            request_id,
            started_at_ms,
            config_revision,
            client_ip,
            method,
            path,
            uri,
            http_version,
            request_headers,
            request_body,
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
            response_headers: Vec::new(),
            gateway_auth_rejected: false,
            pending: true,
        }
    }

    pub(super) fn set_response(&mut self, status_code: u16, headers: Vec<HttpHeader>) {
        self.status_code = Some(status_code);
        self.response_headers = headers;
    }

    pub(super) fn mark_gateway_auth_rejected(&mut self) {
        self.gateway_auth_rejected = true;
    }

    pub(super) fn exclude(&mut self) {
        self.pending = false;
    }

    fn finish(&mut self, outcome: HttpAccessLogOutcome, mut response_body: BodyCapture) {
        if !self.pending {
            return;
        }
        self.pending = false;
        if !should_record(
            &self.metadata.path,
            self.metadata.client_ip,
            self.status_code,
            outcome,
        ) {
            return;
        }
        response_body.finish(outcome == HttpAccessLogOutcome::Completed);
        let response_body = response_body.take_snapshot();
        let duration_ms = u64::try_from(self.started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let log = HttpAccessLog {
            request_id: self.metadata.request_id,
            started_at_ms: self.metadata.started_at_ms,
            config_revision: self.metadata.config_revision,
            client_ip: self.metadata.client_ip,
            method: std::mem::take(&mut self.metadata.method),
            path: std::mem::take(&mut self.metadata.path),
            uri: std::mem::take(&mut self.metadata.uri),
            http_version: self.metadata.http_version,
            status_code: self.status_code,
            duration_ms,
            response_bytes: response_body.total_bytes,
            outcome,
            gateway_auth_rejected: self.gateway_auth_rejected,
            exchange: Some(HttpAccessLogExchange {
                request_headers: std::mem::take(&mut self.metadata.request_headers),
                request_body: self.metadata.request_body.take_snapshot(),
                response_headers: std::mem::take(&mut self.response_headers),
                response_body,
            }),
        };
        let notification = change_notification(&log);
        self.telemetry
            .try_record_http_access(log, &self.settings, notification);
    }
}

impl Drop for AccessLogCompletion {
    fn drop(&mut self) {
        self.finish(HttpAccessLogOutcome::Cancelled, BodyCapture::new(false));
    }
}

pub(super) struct AccessLogBody {
    inner: Body,
    completion: Option<AccessLogCompletion>,
    capture: BodyCapture,
}

impl AccessLogBody {
    pub(super) fn new(inner: Body, completion: AccessLogCompletion) -> Self {
        let complete = inner.is_end_stream();
        Self {
            inner,
            completion: Some(completion),
            capture: BodyCapture::new(complete),
        }
    }

    fn finish(&mut self, outcome: HttpAccessLogOutcome) {
        if let Some(mut completion) = self.completion.take() {
            let capture = std::mem::replace(&mut self.capture, BodyCapture::new(false));
            completion.finish(outcome, capture);
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
                    self.capture.observe(data);
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
