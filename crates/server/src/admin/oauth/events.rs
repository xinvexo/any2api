use std::{convert::Infallible, time::Duration};

use axum::{
    Router,
    extract::State,
    http::{HeaderValue, StatusCode},
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::get,
};
use futures_util::{StreamExt, stream};
use tokio::sync::watch;

use crate::{
    admin::error::AdminApiError, http_access_log::ExcludeFromHttpAccessLog, state::AppState,
};

const QUOTA_CHANGED: &str = "oauth_quota_changed";
const REFRESH_DIAGNOSTIC_CHANGED: &str = "oauth_refresh_diagnostic_changed";
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/oauth/quota-events", get(subscribe))
}

async fn subscribe(State(state): State<AppState>) -> Result<Response, AdminApiError> {
    let oauth = state.oauth().ok_or_else(oauth_unavailable)?;
    let lifecycle = state.runtime().lifecycle().clone();
    let quota =
        change_notifications(oauth.subscribe_quota_changes()).map(|epoch| (QUOTA_CHANGED, epoch));
    let refresh = change_notifications(oauth.subscribe_refresh_failure_changes())
        .map(|epoch| (REFRESH_DIAGNOSTIC_CHANGED, epoch));
    let stream = stream::select(quota, refresh)
        .take_until(async move { lifecycle.draining().await })
        .map(|(event, epoch)| {
            Ok::<_, Infallible>(Event::default().event(event).data(epoch.to_string()))
        });
    let mut response = Sse::new(stream)
        .keep_alive(
            KeepAlive::new()
                .interval(KEEP_ALIVE_INTERVAL)
                .text("keep-alive"),
        )
        .into_response();
    response
        .headers_mut()
        .insert("x-accel-buffering", HeaderValue::from_static("no"));
    response.extensions_mut().insert(ExcludeFromHttpAccessLog);
    Ok(response)
}

fn change_notifications(receiver: watch::Receiver<u64>) -> impl futures_util::Stream<Item = u64> {
    stream::unfold((receiver, true), |(mut receiver, initial)| async move {
        if !initial && receiver.changed().await.is_err() {
            return None;
        }
        let epoch = *receiver.borrow_and_update();
        Some((epoch, (receiver, false)))
    })
}

fn oauth_unavailable() -> AdminApiError {
    AdminApiError::new(
        StatusCode::SERVICE_UNAVAILABLE,
        "oauth_unavailable",
        "OAuth2 login is unavailable",
    )
}

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;

    use super::change_notifications;

    #[tokio::test]
    async fn stream_starts_with_current_epoch_and_then_emits_changes() {
        let (sender, receiver) = tokio::sync::watch::channel(4);
        let stream = change_notifications(receiver);
        futures_util::pin_mut!(stream);
        assert_eq!(stream.next().await, Some(4));
        sender.send_replace(5);
        assert_eq!(stream.next().await, Some(5));
    }
}
