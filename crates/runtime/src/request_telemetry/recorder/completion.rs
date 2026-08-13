use any2api_domain::{
    CompletedRequestLog, ErrorClass, RequestAttempt, RequestLog, bound_error_message,
};

use super::request::{
    CANCELLED_STATUS_CODE, RequestRecorderInner, bound_optional_error_message, duration_ms,
};

impl RequestRecorderInner {
    pub(super) fn finish(
        &self,
        status_code: u16,
        error_class: Option<ErrorClass>,
        error_message: Option<String>,
    ) {
        let record = {
            let mut state = self.state.lock().expect("request recorder state");
            if state.finished {
                return;
            }
            state.finished = true;
            let final_target = state.final_target;
            let observation = state.observation;
            let token_usage = observation.token_usage();
            let quota_cost = state
                .quota_cost_rate
                .as_ref()
                .and_then(|rate| rate.estimate(token_usage));
            let attempts = std::mem::take(&mut state.attempts);
            let error_class = final_error_class(&attempts, error_class);
            let error_message = final_error_message(&attempts, error_message);
            CompletedRequestLog {
                request: RequestLog {
                    request_id: self.request_id,
                    started_at_ms: self.started_at_ms,
                    client_ip: self.client_ip,
                    config_revision: self.config_revision,
                    gateway_api_key_id: Some(self.gateway_api_key_id),
                    ingress_protocol: self.operation.dialect(),
                    operation: self.operation,
                    public_model: state.public_model.clone(),
                    thinking_level: state.thinking_level.clone(),
                    provider_endpoint_id: final_target.and_then(|target| target.endpoint_id),
                    credential_id: final_target.and_then(|target| target.credential_id),
                    oauth_account_id: final_target.and_then(|target| target.oauth_account_id),
                    proxy_profile_id: final_target.map(|target| target.proxy_id),
                    status_code,
                    error_class,
                    error_message,
                    attempt_count: u32::try_from(attempts.len()).unwrap_or(u32::MAX),
                    latency_ms: duration_ms(self.started_at.elapsed()),
                    first_token_ms: observation.first_token_ms(),
                    input_tokens: token_usage.input_tokens(),
                    output_tokens: token_usage.output_tokens(),
                    cache_read_tokens: token_usage.cache_read_tokens(),
                    quota_cost,
                    is_stream: state.is_stream,
                },
                attempts,
                telemetry_position: None,
            }
        };
        self.telemetry.try_record(record, self.policy);
    }
}

impl Drop for RequestRecorderInner {
    fn drop(&mut self) {
        self.finish(
            CANCELLED_STATUS_CODE,
            Some(ErrorClass::Cancelled),
            Some(bound_error_message("request cancelled")),
        );
    }
}

fn final_error_class(
    attempts: &[RequestAttempt],
    fallback: Option<ErrorClass>,
) -> Option<ErrorClass> {
    match (
        attempts.last().and_then(|attempt| attempt.error_class),
        fallback,
    ) {
        (Some(ErrorClass::Cancelled), Some(fallback)) if fallback != ErrorClass::Cancelled => {
            Some(fallback)
        }
        (Some(error_class), _) => Some(error_class),
        (None, fallback) => fallback,
    }
}

fn final_error_message(attempts: &[RequestAttempt], fallback: Option<String>) -> Option<String> {
    // Prefer the explicit request-level safe diagnostic when finish provides one.
    // Fall back to the last attempt diagnostic for stream/drop paths.
    let selected = match (
        attempts
            .last()
            .and_then(|attempt| attempt.error_message.clone()),
        fallback,
    ) {
        (_, Some(message)) => Some(message),
        (Some(message), None) => Some(message),
        (None, None) => None,
    };
    selected.and_then(bound_optional_error_message)
}

#[cfg(test)]
mod tests;
