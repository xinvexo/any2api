use std::time::Duration;

use axum::{body::Body, middleware::Next, response::Response};
use tower_http::timeout::TimeoutBody;

pub(crate) async fn apply(
    request: axum::extract::Request,
    next: Next,
    timeout: Duration,
) -> Response {
    let (parts, body) = request.into_parts();
    let body = Body::new(TimeoutBody::new(timeout, body));
    next.run(axum::extract::Request::from_parts(parts, body))
        .await
}

#[cfg(test)]
mod tests {
    use std::{convert::Infallible, time::Duration};

    use axum::body::Bytes;
    use http_body_util::BodyExt;
    use tokio::time::{sleep, timeout};
    use tower_http::timeout::TimeoutBody;

    #[tokio::test]
    async fn timeout_body_is_idle_between_frames() {
        let body = futures_util::stream::iter([
            Ok::<_, Infallible>(http_body::Frame::data(Bytes::from_static(b"a"))),
            Ok(http_body::Frame::data(Bytes::from_static(b"b"))),
        ]);
        let body = TimeoutBody::new(
            Duration::from_secs(1),
            http_body_util::StreamBody::new(body),
        );
        let collected = body.collect().await.expect("immediate frames should pass");
        assert_eq!(collected.to_bytes(), Bytes::from_static(b"ab"));

        let delayed = futures_util::stream::once(async {
            sleep(Duration::from_millis(50)).await;
            Ok::<_, Infallible>(http_body::Frame::data(Bytes::from_static(b"late")))
        });
        let delayed = TimeoutBody::new(
            Duration::from_millis(5),
            http_body_util::StreamBody::new(delayed),
        );
        let error = timeout(Duration::from_secs(1), delayed.collect())
            .await
            .expect("timeout body should resolve")
            .expect_err("delayed body should time out");
        assert!(error.to_string().contains("designated timeout"));
    }
}
