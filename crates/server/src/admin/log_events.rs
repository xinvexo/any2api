use std::{convert::Infallible, pin::Pin, sync::Arc, time::Duration};

use axum::{
    Router,
    extract::State,
    http::HeaderValue,
    response::{
        IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
    routing::get,
};
use futures_util::{
    StreamExt,
    stream::{self, Stream},
};
use tokio::sync::watch;

use crate::{http_access_log::ExcludeFromHttpAccessLog, state::AppState};

use super::realtime::OverviewSnapshot;

const REQUEST_LOGS_CHANGED: &str = "request_logs_changed";
const ACTIVE_REQUESTS_CHANGED: &str = "active_requests_changed";
const SYSTEM_LOGS_CHANGED: &str = "system_logs_changed";
const OAUTH_QUOTA_CHANGED: &str = "oauth_quota_changed";
const OAUTH_REFRESH_DIAGNOSTIC_CHANGED: &str = "oauth_refresh_diagnostic_changed";
const OVERVIEW_SNAPSHOT: &str = "overview_snapshot";
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/events", get(subscribe))
}

async fn subscribe(State(state): State<AppState>) -> Response {
    let telemetry = state.request_telemetry();
    let overview = state.admin_realtime().subscribe();
    let oauth_changes = state.oauth().map(|oauth| {
        (
            oauth.subscribe_quota_changes(),
            oauth.subscribe_refresh_failure_changes(),
        )
    });
    let lifecycle = state.runtime().lifecycle();
    let stream = realtime_notifications(
        telemetry.subscribe_request_log_changes(),
        telemetry.subscribe_active_request_changes(),
        telemetry.subscribe_http_access_log_changes(),
        overview,
        oauth_changes,
    )
    .take_until(async move { lifecycle.draining().await })
    .map(|notification| Ok::<_, Infallible>(into_sse_event(notification)));
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
    response
}

#[derive(Debug, Eq, PartialEq)]
enum RealtimeNotification<T> {
    LogChanged(LogChange),
    Overview(T),
}

#[derive(Debug, Eq, PartialEq)]
struct LogChange {
    name: &'static str,
    epoch: u64,
}

fn into_sse_event(notification: RealtimeNotification<Arc<OverviewSnapshot>>) -> Event {
    match notification {
        RealtimeNotification::LogChanged(change) => Event::default()
            .event(change.name)
            .data(change.epoch.to_string()),
        RealtimeNotification::Overview(snapshot) => Event::default().event(OVERVIEW_SNAPSHOT).data(
            serde_json::to_string(snapshot.as_ref())
                .expect("overview snapshot contains only JSON-safe values"),
        ),
    }
}

fn realtime_notifications<T>(
    request_logs: watch::Receiver<u64>,
    active_requests: watch::Receiver<u64>,
    system_logs: watch::Receiver<u64>,
    overview: watch::Receiver<Option<T>>,
    oauth_changes: Option<(watch::Receiver<u64>, watch::Receiver<u64>)>,
) -> impl Stream<Item = RealtimeNotification<T>>
where
    T: Clone + Send + Sync + 'static,
{
    stream::select(
        stream::select(
            log_change_notifications(request_logs, active_requests, system_logs),
            oauth_change_notifications(oauth_changes),
        )
        .map(RealtimeNotification::LogChanged),
        overview_notifications(overview).map(RealtimeNotification::Overview),
    )
}

fn oauth_change_notifications(
    receivers: Option<(watch::Receiver<u64>, watch::Receiver<u64>)>,
) -> Pin<Box<dyn Stream<Item = LogChange> + Send>> {
    let Some((quota, refresh_diagnostic)) = receivers else {
        return Box::pin(stream::pending());
    };
    Box::pin(stream::select(
        change_notifications(quota).map(|epoch| LogChange {
            name: OAUTH_QUOTA_CHANGED,
            epoch,
        }),
        change_notifications(refresh_diagnostic).map(|epoch| LogChange {
            name: OAUTH_REFRESH_DIAGNOSTIC_CHANGED,
            epoch,
        }),
    ))
}

fn change_notifications(receiver: watch::Receiver<u64>) -> impl Stream<Item = u64> {
    stream::unfold((receiver, true), |(mut receiver, initial)| async move {
        if !initial && receiver.changed().await.is_err() {
            return None;
        }
        let epoch = *receiver.borrow_and_update();
        Some((epoch, (receiver, false)))
    })
}

fn log_change_notifications(
    request_logs: watch::Receiver<u64>,
    active_requests: watch::Receiver<u64>,
    system_logs: watch::Receiver<u64>,
) -> impl Stream<Item = LogChange> {
    stream::unfold(
        (request_logs, active_requests, system_logs, 0_u8),
        |(mut request_logs, mut active_requests, mut system_logs, initial)| async move {
            let (name, epoch, next_initial) = match initial {
                0 => (REQUEST_LOGS_CHANGED, *request_logs.borrow_and_update(), 1),
                1 => (
                    ACTIVE_REQUESTS_CHANGED,
                    *active_requests.borrow_and_update(),
                    2,
                ),
                2 => (SYSTEM_LOGS_CHANGED, *system_logs.borrow_and_update(), 3),
                _ => {
                    tokio::select! {
                        changed = request_logs.changed() => {
                            changed.ok()?;
                            (REQUEST_LOGS_CHANGED, *request_logs.borrow_and_update(), 3)
                        }
                        changed = active_requests.changed() => {
                            changed.ok()?;
                            (ACTIVE_REQUESTS_CHANGED, *active_requests.borrow_and_update(), 3)
                        }
                        changed = system_logs.changed() => {
                            changed.ok()?;
                            (SYSTEM_LOGS_CHANGED, *system_logs.borrow_and_update(), 3)
                        }
                    }
                }
            };
            Some((
                LogChange { name, epoch },
                (request_logs, active_requests, system_logs, next_initial),
            ))
        },
    )
}

fn overview_notifications<T>(receiver: watch::Receiver<Option<T>>) -> impl Stream<Item = T>
where
    T: Clone + Send + Sync + 'static,
{
    stream::unfold((receiver, true), |(mut receiver, mut initial)| async move {
        loop {
            if !initial && receiver.changed().await.is_err() {
                return None;
            }
            initial = false;
            let snapshot = receiver.borrow_and_update().clone();
            if let Some(snapshot) = snapshot {
                return Some((snapshot, (receiver, false)));
            }
        }
    })
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use futures_util::StreamExt;

    use super::{
        ACTIVE_REQUESTS_CHANGED, OAUTH_QUOTA_CHANGED, OAUTH_REFRESH_DIAGNOSTIC_CHANGED,
        REQUEST_LOGS_CHANGED, RealtimeNotification, SYSTEM_LOGS_CHANGED, realtime_notifications,
    };

    #[tokio::test]
    async fn stream_starts_with_current_epochs_and_snapshot_then_emits_changes() {
        let (request_sender, request_receiver) = tokio::sync::watch::channel(4);
        let (_active_sender, active_receiver) = tokio::sync::watch::channel(6);
        let (_system_sender, system_receiver) = tokio::sync::watch::channel(7);
        let (overview_sender, overview_receiver) = tokio::sync::watch::channel(Some(10_u64));
        let (_quota_sender, quota_receiver) = tokio::sync::watch::channel(8);
        let (_diagnostic_sender, diagnostic_receiver) = tokio::sync::watch::channel(9);
        let stream = realtime_notifications(
            request_receiver,
            active_receiver,
            system_receiver,
            overview_receiver,
            Some((quota_receiver, diagnostic_receiver)),
        );
        futures_util::pin_mut!(stream);

        let mut epochs = HashMap::new();
        let mut overview = None;
        for _ in 0..6 {
            match stream.next().await.expect("initial realtime event") {
                RealtimeNotification::LogChanged(change) => {
                    epochs.insert(change.name, change.epoch);
                }
                RealtimeNotification::Overview(snapshot) => overview = Some(snapshot),
            }
        }
        assert_eq!(epochs.get(REQUEST_LOGS_CHANGED), Some(&4));
        assert_eq!(epochs.get(ACTIVE_REQUESTS_CHANGED), Some(&6));
        assert_eq!(epochs.get(SYSTEM_LOGS_CHANGED), Some(&7));
        assert_eq!(epochs.get(OAUTH_QUOTA_CHANGED), Some(&8));
        assert_eq!(epochs.get(OAUTH_REFRESH_DIAGNOSTIC_CHANGED), Some(&9));
        assert_eq!(overview, Some(10));

        request_sender.send_replace(5);
        overview_sender.send_replace(Some(11));
        let mut saw_request = false;
        let mut saw_overview = false;
        for _ in 0..2 {
            match stream.next().await.expect("updated realtime event") {
                RealtimeNotification::LogChanged(change) => {
                    saw_request = change.name == REQUEST_LOGS_CHANGED && change.epoch == 5;
                }
                RealtimeNotification::Overview(snapshot) => saw_overview = snapshot == 11,
            }
        }
        assert!(saw_request);
        assert!(saw_overview);
    }
}
