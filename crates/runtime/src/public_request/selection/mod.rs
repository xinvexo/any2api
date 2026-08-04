mod filter_recorder;
mod fixed;
mod generation;
mod selector;
#[cfg(test)]
mod tests;

pub(super) use selector::{
    CandidateSelector, FixedSelectionError, GenerationSelection, no_available_credentials,
    rate_limit_error, rate_limited, select_candidate, select_fixed_candidate,
    temporarily_unavailable,
};

#[cfg(test)]
use super::SelectedCandidate;
#[cfg(test)]
use crate::routing::RouteCandidate;
#[cfg(test)]
use selector::{
    select_generation_candidate, try_select_fixed_candidate_for_test,
    try_select_generation_candidate_for_test, wait_for_generation_candidate,
};
