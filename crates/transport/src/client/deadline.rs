use std::{future::Future, time::Duration};

use tokio::{
    sync::oneshot,
    time::{Instant, timeout, timeout_at},
};

use crate::error::TransportError;

enum ConnectPhase<T, E> {
    SendFinished(Result<T, E>),
    BodyStarted(Result<(), oneshot::error::RecvError>),
}

pub(super) async fn await_response_headers<F, T, E, M>(
    send: F,
    body_started: oneshot::Receiver<()>,
    connect_deadline: Instant,
    read_timeout: Duration,
    map_send_error: M,
    connect_timeout_error: TransportError,
    read_timeout_error: TransportError,
) -> Result<T, TransportError>
where
    F: Future<Output = Result<T, E>>,
    M: Fn(E) -> TransportError,
{
    tokio::pin!(send);
    let phase = timeout_at(connect_deadline, async {
        tokio::select! {
            biased;
            result = &mut send => ConnectPhase::SendFinished(result),
            signal = body_started => ConnectPhase::BodyStarted(signal),
        }
    })
    .await
    .map_err(|_| connect_timeout_error.clone())?;

    match phase {
        ConnectPhase::SendFinished(result) => result.map_err(map_send_error),
        ConnectPhase::BodyStarted(Ok(())) => timeout(read_timeout, &mut send)
            .await
            .map_err(|_| read_timeout_error)?
            .map_err(map_send_error),
        ConnectPhase::BodyStarted(Err(_)) => timeout_at(connect_deadline, &mut send)
            .await
            .map_err(|_| connect_timeout_error)?
            .map_err(map_send_error),
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use any2api_domain::RetrySafety;
    use tokio::{sync::oneshot, time::Instant};

    use super::await_response_headers;
    use crate::error::{TransportError, TransportErrorStage, TransportFailureScope};

    #[tokio::test]
    async fn closed_body_signal_stays_in_the_connect_phase() {
        let (sender, receiver) = oneshot::channel();
        drop(sender);

        let response = await_response_headers(
            async {
                tokio::time::sleep(Duration::from_millis(20)).await;
                Ok::<_, ()>(7)
            },
            receiver,
            Instant::now() + Duration::from_millis(100),
            Duration::from_millis(1),
            |_| test_error("send"),
            test_error("connect"),
            test_error("read"),
        )
        .await
        .expect("closed sender must not start the read timeout");

        assert_eq!(response, 7);
    }

    fn test_error(message: &str) -> TransportError {
        TransportError::new(
            TransportErrorStage::AwaitHeaders,
            TransportFailureScope::Unattributed,
            RetrySafety::Ambiguous,
            message,
        )
    }
}
