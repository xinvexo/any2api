use std::{
    collections::HashMap,
    time::{Duration, SystemTime},
};

use any2api_domain::{ProtocolOperation, RetryAfterHint, RoutingCredentialId};
use any2api_protocol::api::RequestExecutionProfile;
use tokio::time::Instant;

use crate::{health::ReliabilityPolicy, public_request::execution_limits};

pub(super) struct RetryBudget {
    policy: ReliabilityPolicy,
    deadline: Instant,
    pub(super) attempts: u32,
    switches: u32,
    last_credential: Option<RoutingCredentialId>,
    pub(super) attempts_by_credential: HashMap<RoutingCredentialId, u32>,
}

impl RetryBudget {
    pub(super) fn new(
        policy: ReliabilityPolicy,
        operation: ProtocolOperation,
        profile: RequestExecutionProfile,
    ) -> Self {
        Self {
            policy,
            deadline: Instant::now()
                + execution_limits::retry_budget(operation, profile, policy.precommit_total_budget),
            attempts: 0,
            switches: 0,
            last_credential: None,
            attempts_by_credential: HashMap::new(),
        }
    }

    pub(super) fn remaining(&self) -> Duration {
        self.deadline.saturating_duration_since(Instant::now())
    }

    pub(super) fn deadline(&self) -> Instant {
        self.deadline
    }

    pub(super) fn register_attempt(&mut self, credential_id: RoutingCredentialId) -> Option<u32> {
        if !self.can_register_attempt(credential_id) {
            return None;
        }
        if self
            .last_credential
            .is_some_and(|previous| previous != credential_id)
        {
            self.switches += 1;
        }
        let prior = self
            .attempts_by_credential
            .get(&credential_id)
            .copied()
            .unwrap_or(0);
        self.attempts_by_credential.insert(credential_id, prior + 1);
        self.last_credential = Some(credential_id);
        self.attempts += 1;
        Some(self.attempts)
    }

    pub(super) fn can_register_attempt(&self, credential_id: RoutingCredentialId) -> bool {
        if self.attempts >= self.policy.max_total_attempts || self.remaining().is_zero() {
            return false;
        }
        if self
            .last_credential
            .is_some_and(|previous| previous != credential_id)
            && self.switches >= self.policy.max_credential_switches
        {
            return false;
        }
        self.attempts_by_credential
            .get(&credential_id)
            .copied()
            .unwrap_or(0)
            <= self.policy.max_same_credential_retries
    }

    pub(super) fn can_retry(&self) -> bool {
        self.attempts < self.policy.max_total_attempts && !self.remaining().is_zero()
    }

    pub(super) fn can_wait(&self, delay: Duration) -> bool {
        delay < self.remaining()
    }

    pub(super) fn next_delay(
        &self,
        credential_id: RoutingCredentialId,
        retry_after: Option<RetryAfterHint>,
    ) -> Duration {
        let credential_attempts = self
            .attempts_by_credential
            .get(&credential_id)
            .copied()
            .expect("retry delay follows a registered credential attempt");
        let exponent = credential_attempts.saturating_sub(1).min(31);
        let multiplier = 1_u32 << exponent;
        let base = self
            .policy
            .base_delay
            .saturating_mul(multiplier)
            .min(self.policy.max_delay);
        let fallback = jitter(base, self.policy.jitter_ratio);
        let retry_after = retry_after
            .map(|hint| hint.delay_from(SystemTime::now()))
            .unwrap_or_default();
        fallback.max(retry_after)
    }
}

/// Retry jitter only needs de-synchronization, not unpredictability, so the
/// wall-clock sub-second nanos stand in for a real RNG dependency.
fn jitter(delay: Duration, ratio: u32) -> Duration {
    if ratio == 0 || delay.is_zero() {
        return delay;
    }
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |value| value.subsec_nanos());
    let width = ratio.saturating_mul(2).saturating_add(1);
    let offset = (nanos % width) as i64 - i64::from(ratio);
    let percent = (100_i64 + offset).max(0) as u32;
    delay.saturating_mul(percent) / 100
}

#[cfg(test)]
mod tests {
    use any2api_domain::{CredentialId, SettingsConfiguration};

    use super::*;

    #[tokio::test(start_paused = true)]
    async fn retry_delay_is_isolated_by_credential() {
        let mut policy =
            ReliabilityPolicy::from_settings(SettingsConfiguration::defaults().reliability());
        policy.max_total_attempts = 10;
        policy.max_credential_switches = 10;
        policy.max_same_credential_retries = 3;
        policy.precommit_total_budget = Duration::from_secs(60);
        policy.base_delay = Duration::from_secs(1);
        policy.max_delay = Duration::from_secs(8);
        policy.jitter_ratio = 0;
        let mut budget = RetryBudget::new(
            policy,
            ProtocolOperation::Responses,
            RequestExecutionProfile::Standard,
        );
        let first = CredentialId::new().into();
        let second = CredentialId::new().into();

        assert_eq!(budget.register_attempt(first), Some(1));
        assert_eq!(budget.next_delay(first, None), Duration::from_secs(1));
        assert_eq!(budget.register_attempt(first), Some(2));
        assert_eq!(budget.next_delay(first, None), Duration::from_secs(2));
        assert_eq!(budget.register_attempt(second), Some(3));
        assert_eq!(budget.next_delay(second, None), Duration::from_secs(1));
    }

    #[tokio::test(start_paused = true)]
    async fn retry_after_is_a_non_jittered_minimum_and_must_fit_the_budget() {
        let mut policy =
            ReliabilityPolicy::from_settings(SettingsConfiguration::defaults().reliability());
        policy.precommit_total_budget = Duration::from_secs(10);
        policy.base_delay = Duration::from_secs(2);
        policy.max_delay = Duration::from_secs(2);
        policy.jitter_ratio = 100;
        let mut budget = RetryBudget::new(
            policy,
            ProtocolOperation::Responses,
            RequestExecutionProfile::Standard,
        );
        let credential = CredentialId::new().into();
        assert_eq!(budget.register_attempt(credential), Some(1));

        assert_eq!(
            budget.next_delay(
                credential,
                Some(RetryAfterHint::Delay(Duration::from_secs(7)))
            ),
            Duration::from_secs(7)
        );
        assert!(budget.can_wait(Duration::from_secs(9)));
        assert!(!budget.can_wait(Duration::from_secs(10)));
        tokio::time::advance(Duration::from_secs(3)).await;
        assert!(!budget.can_wait(Duration::from_secs(7)));
    }

    #[test]
    fn same_credential_retry_limit_is_checked_before_attempt_mutation() {
        let mut policy =
            ReliabilityPolicy::from_settings(SettingsConfiguration::defaults().reliability());
        policy.max_total_attempts = 10;
        policy.max_same_credential_retries = 1;
        policy.precommit_total_budget = Duration::from_secs(60);
        let mut budget = RetryBudget::new(
            policy,
            ProtocolOperation::Responses,
            RequestExecutionProfile::Standard,
        );
        let credential = CredentialId::new().into();

        assert_eq!(budget.register_attempt(credential), Some(1));
        assert_eq!(budget.register_attempt(credential), Some(2));
        assert!(!budget.can_register_attempt(credential));
        assert_eq!(budget.register_attempt(credential), None);
        assert_eq!(budget.attempts, 2);
        assert_eq!(budget.attempts_by_credential[&credential], 2);
    }
}
