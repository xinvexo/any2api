use std::time::Instant;

use any2api_domain::UpstreamError;
use any2api_protocol::api::ProtocolUpstreamFailureEvidence;
use any2api_provider::api::UpstreamResponseMeta;
use futures_util::StreamExt;
use http::StatusCode;
use tokio::time::timeout;

use super::{GuardedBody, StreamPrimeFailure};

impl GuardedBody {
    #[cfg(test)]
    pub(super) async fn prime(self) -> Result<Self, any2api_domain::PublicError> {
        let mut body = self
            .prime_attempt()
            .await
            .map_err(StreamPrimeFailure::into_public)?;
        body.commit_precommit_continuation(None)?;
        Ok(body)
    }

    pub(in crate::public_request) async fn prime_attempt(
        mut self,
    ) -> Result<Self, StreamPrimeFailure> {
        let deadline = Instant::now() + self.precommit_budget.max_duration();
        self.precommit_deadline = Some(deadline);
        loop {
            if self.precommit_commit_ready
                || self.precommit_upstream_failure.is_some()
                || self.pending_error.is_some()
            {
                break;
            }
            if self.process_buffered_frame(Some(deadline)) {
                continue;
            }
            if self.upstream_done {
                self.finish_decoder(Some(deadline));
                if self.precommit_commit_ready
                    || self.precommit_upstream_failure.is_some()
                    || self.pending_error.is_some()
                {
                    continue;
                }
                break;
            }
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                self.set_timeout_error();
                break;
            }
            match timeout(remaining, self.upstream.next()).await {
                Ok(Some(Ok(chunk))) => self.process_chunk(chunk, Some(deadline)),
                Ok(Some(Err(error))) => self.set_transport_error(&error),
                Ok(None) => self.process_eof(Some(deadline)),
                Err(_) => self.set_timeout_error(),
            }
        }
        if let Some(failure) = self.precommit_upstream_failure.take() {
            self.finish_precommit_upstream_failure(&failure.error);
            return Err(StreamPrimeFailure::Upstream(failure));
        }
        if self.pending_error.is_some() && !self.precommit_commit_ready {
            return Err(StreamPrimeFailure::Public(self.finish_precommit_failure()));
        }
        if self.pending.is_empty() {
            return Err(StreamPrimeFailure::Public(self.finish_precommit_failure()));
        }
        if let Err(error) = self.commit_precommit_frames(Some(deadline)) {
            self.set_pending_error(error);
            return Err(StreamPrimeFailure::Public(self.finish_precommit_failure()));
        }
        Ok(self)
    }

    pub(super) fn classify_upstream_failure(
        &self,
        evidence: &ProtocolUpstreamFailureEvidence,
        allow_retry_safety_override: bool,
    ) -> UpstreamError {
        let status = StatusCode::from_u16(self.status_code).expect("upstream status is valid");
        let body = evidence.raw_json();
        let classified = self.driver.classify_error(
            self.upstream_operation,
            &UpstreamResponseMeta {
                status,
                headers: self.upstream_headers.clone(),
            },
            &body[..body
                .len()
                .min(super::super::response::MAX_UPSTREAM_ERROR_BODY_BYTES)],
        );
        match (
            allow_retry_safety_override,
            evidence.retry_safety_override(),
        ) {
            (true, Some(safety)) => classified.with_retry_safety(safety),
            _ => classified,
        }
    }
}

#[cfg(test)]
impl StreamPrimeFailure {
    fn into_public(self) -> any2api_domain::PublicError {
        match self {
            Self::Public(error) => error,
            Self::Upstream(_) => super::super::response::public_error(
                any2api_domain::PublicErrorCode::UpstreamError,
                "upstream stream reported a failure before semantic output",
            ),
        }
    }
}
