use any2api_domain::RequestTelemetryPosition;
use any2api_provider::api::OAuthQuotaWindow;

use super::state::{QuotaObservationAnchor, QuotaWindowKey, QuotaWindowState};

pub(super) fn official_reset(state: &QuotaWindowState, window: &OAuthQuotaWindow) -> bool {
    state.anchor.reset_at != window.reset_at || window.used_percent < state.anchor.used_percent
}

pub(super) fn new_window(
    key: QuotaWindowKey,
    window: &OAuthQuotaWindow,
    position: RequestTelemetryPosition,
) -> QuotaWindowState {
    QuotaWindowState {
        key,
        anchor: anchor(window, position),
        total_delta_used_percent: 0.0,
        total_local_cost_credits: 0.0,
        completed_interval_count: 0,
    }
}

pub(super) fn rollover_window(
    mut previous: QuotaWindowState,
    window: &OAuthQuotaWindow,
    position: RequestTelemetryPosition,
) -> QuotaWindowState {
    previous.anchor = anchor(window, position);
    previous
}

pub(super) fn anchor(
    window: &OAuthQuotaWindow,
    position: RequestTelemetryPosition,
) -> QuotaObservationAnchor {
    QuotaObservationAnchor {
        used_percent: window.used_percent,
        reset_at: window.reset_at,
        position,
    }
}
