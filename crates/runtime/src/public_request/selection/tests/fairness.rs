use std::{cell::Cell, collections::BTreeMap, sync::Arc};

use super::super::{GenerationSelection, generation::try_select_for_test_with_cursor};
use super::selection::{candidate, unlimited_candidate};
use crate::{
    public_request::SelectedCandidate,
    routing::{RouteCandidate, SchedulerEpoch},
};

#[tokio::test(start_paused = true)]
async fn stable_ring_skips_rate_limited_slots_without_biasing_healthy_credentials() {
    let epoch = SchedulerEpoch::new();
    let first = unlimited_candidate("first", 1, Arc::clone(&epoch), 0);
    let blocked = candidate("blocked", 2, Arc::clone(&epoch), 0);
    let third = unlimited_candidate("third", 3, Arc::clone(&epoch), 0);
    drop(blocked.binding.try_reserve().expect("exhaust blocked RPM"));
    let tiers = BTreeMap::from([(0, vec![first.clone(), blocked.clone(), third.clone()])]);
    let cursor = Cell::new(0_u64);
    let mut first_count = 0;
    let mut third_count = 0;

    for _ in 0..100 {
        let selected = select_with_cursor(&tiers, &cursor);
        if selected.candidate.credential_id == first.credential_id {
            first_count += 1;
        } else if selected.candidate.credential_id == third.credential_id {
            third_count += 1;
        } else {
            panic!("rate-limited credential must not be selected");
        }
        drop(selected);
    }

    assert_eq!(first_count, 50);
    assert_eq!(third_count, 50);
    tokio::time::advance(std::time::Duration::from_secs(60)).await;
    let recovered = (0..3)
        .map(|_| select_with_cursor(&tiers, &cursor))
        .any(|selected| selected.candidate.credential_id == blocked.credential_id);
    assert!(
        recovered,
        "the recovered slot must rejoin the unchanged ring"
    );
}

fn select_with_cursor(
    tiers: &BTreeMap<u16, Vec<RouteCandidate>>,
    cursor: &Cell<u64>,
) -> Box<SelectedCandidate> {
    match try_select_for_test_with_cursor(
        false,
        tiers,
        |_| {
            let reserved = cursor.get();
            cursor.set(reserved.wrapping_add(1));
            Some(reserved)
        },
        |_, skipped| {
            cursor.set(cursor.get().wrapping_add(skipped));
            true
        },
    )
    .expect("generation selection")
    {
        GenerationSelection::Acquired(selected) => selected,
        GenerationSelection::RateLimited(_) => panic!("healthy credentials are unlimited"),
        GenerationSelection::TemporarilyUnavailable(_) => {
            panic!("healthy credentials are available")
        }
        GenerationSelection::RetryDeferred(_) => panic!("no retry deferral exists"),
        GenerationSelection::NoCandidates => panic!("tier has candidates"),
    }
}
