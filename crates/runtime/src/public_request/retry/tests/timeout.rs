use std::time::Duration;

use tokio::time::Instant;

use crate::{public_request::retry::within_attempt_budget, request_telemetry::AttemptRecorder};

#[tokio::test(start_paused = true)]
async fn attempt_result_at_the_deadline_wins_over_the_outer_timeout() {
    let deadline = Instant::now() + Duration::from_millis(25);
    let marker = AttemptRecorder::disabled().timeout_marker();
    let attempt = async {
        tokio::time::sleep_until(deadline).await;
        7
    };

    match within_attempt_budget(deadline, marker, attempt).await {
        Ok(value) => assert_eq!(value, 7),
        Err(_) => panic!("the completed attempt must win at the shared deadline"),
    }
}
