use std::{convert::Infallible, time::Duration};

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

const REQUEST_LOGS_CHANGED: &str = "request_logs_changed";
const ACTIVE_REQUESTS_CHANGED: &str = "active_requests_changed";
const SYSTEM_LOGS_CHANGED: &str = "system_logs_changed";
const KEEP_ALIVE_INTERVAL: Duration = Duration::from_secs(15);

pub(super) fn routes() -> Router<AppState> {
    Router::new().route("/log-events", get(subscribe))
}

async fn subscribe(State(state): State<AppState>) -> Response {
    let telemetry = state.request_telemetry();
    let lifecycle = state.runtime().lifecycle().clone();
    let stream = change_notifications(
        telemetry.subscribe_request_log_changes(),
        telemetry.subscribe_active_request_changes(),
        telemetry.subscribe_http_access_log_changes(),
    )
    .take_until(async move { lifecycle.draining().await })
    .map(|change| {
        Ok::<_, Infallible>(
            Event::default()
                .event(change.name)
                .data(change.epoch.to_string()),
        )
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
    response
}

#[derive(Debug, Eq, PartialEq)]
struct LogChange {
    name: &'static str,
    epoch: u64,
}

fn change_notifications(
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

#[cfg(test)]
mod tests {
    use futures_util::StreamExt;

    use super::{
        ACTIVE_REQUESTS_CHANGED, LogChange, REQUEST_LOGS_CHANGED, SYSTEM_LOGS_CHANGED,
        change_notifications,
    };

    #[tokio::test]
    async fn stream_starts_with_current_epochs_and_then_emits_changes() {
        let (request_sender, request_receiver) = tokio::sync::watch::channel(4);
        let (active_sender, active_receiver) = tokio::sync::watch::channel(6);
        let (_system_sender, system_receiver) = tokio::sync::watch::channel(7);
        let stream = change_notifications(request_receiver, active_receiver, system_receiver);
        futures_util::pin_mut!(stream);

        let request = stream.next().await.expect("initial request event");
        let active = stream.next().await.expect("initial active request event");
        let system = stream.next().await.expect("initial system event");
        assert_eq!(
            request,
            LogChange {
                name: REQUEST_LOGS_CHANGED,
                epoch: 4,
            }
        );
        assert_eq!(
            active,
            LogChange {
                name: ACTIVE_REQUESTS_CHANGED,
                epoch: 6,
            }
        );
        assert_eq!(
            system,
            LogChange {
                name: SYSTEM_LOGS_CHANGED,
                epoch: 7,
            }
        );

        request_sender.send_replace(5);
        let changed = stream.next().await.expect("changed request event");
        assert_eq!(
            changed,
            LogChange {
                name: REQUEST_LOGS_CHANGED,
                epoch: 5,
            }
        );

        active_sender.send_replace(8);
        let changed = stream.next().await.expect("active request change");
        assert_eq!(
            changed,
            LogChange {
                name: ACTIVE_REQUESTS_CHANGED,
                epoch: 8,
            }
        );
    }
}
