use std::time::Duration;

use any2api_domain::{ProxyProfileId, RetrySafety, UpstreamErrorKind, UpstreamFailureAttribution};
use any2api_transport::api::{TransportError, TransportErrorStage, TransportFailureScope};

use super::support::{attempted_budget, candidate, upstream_failure};
use crate::{
    public_request::{
        retry::decision::{RetryDecision, RetryExclusion, exclude_failed_path, retry_decision},
        upstream::AttemptFailure,
    },
    routing::CandidateExclusions,
};

#[test]
fn credential_model_exclusion_keeps_other_models_and_keys_available() {
    let failed = candidate("failed-model");
    let mut same_credential_other_model = failed.clone();
    same_credential_other_model.upstream_model = "other-model".into();
    same_credential_other_model.target_id = any2api_domain::RouteTargetId::new();
    let other_credential = candidate("other-key");
    let failure = upstream_failure(
        failed.clone(),
        false,
        UpstreamErrorKind::ModelUnavailable,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::CredentialModel,
    );
    let mut exclusions = CandidateExclusions::default();

    exclude_failed_path(&mut exclusions, &failure, RetryExclusion::CredentialModel);

    assert!(!exclusions.allows(&failed));
    assert!(exclusions.allows(&same_credential_other_model));
    assert!(exclusions.allows(&other_credential));
}

#[test]
fn bad_key_exclusion_keeps_another_key_on_the_same_endpoint_available() {
    let failed = candidate("bad-key");
    let mut good = candidate("good-key");
    good.endpoint_id = failed.endpoint_id;
    good.endpoint_config_version = failed.endpoint_config_version;
    good.proxy_id = failed.proxy_id;
    good.proxy_config_version = failed.proxy_config_version;
    let failure = upstream_failure(
        failed.clone(),
        false,
        UpstreamErrorKind::Authentication,
        RetrySafety::RejectedBeforeExecution,
        UpstreamFailureAttribution::Authentication,
    );
    let budget = attempted_budget(failed.credential_id);
    let decision = retry_decision(&failure, &budget, failed.credential_id, false);
    let RetryDecision::Reselect {
        exclusion: scope,
        delay,
    } = decision
    else {
        panic!("bad key must trigger safe reselection");
    };
    assert_eq!(delay, Duration::from_secs(1));
    let mut exclusions = CandidateExclusions::default();

    exclude_failed_path(&mut exclusions, &failure, scope);

    assert!(!exclusions.allows(&failed));
    assert!(exclusions.allows(&good));
}

#[test]
fn egress_exclusion_isolated_by_endpoint_proxy_pair_and_generation() {
    let failed = candidate("bad-egress");
    let mut other_proxy = candidate("other-proxy");
    other_proxy.endpoint_id = failed.endpoint_id;
    other_proxy.endpoint_config_version = failed.endpoint_config_version;
    other_proxy.proxy_id = ProxyProfileId::new();
    let mut other_endpoint = candidate("other-endpoint");
    other_endpoint.proxy_id = failed.proxy_id;
    other_endpoint.proxy_config_version = failed.proxy_config_version;
    let mut new_proxy_generation = failed.clone();
    new_proxy_generation.proxy_config_version += 1;
    let failure = AttemptFailure::Transport {
        error: Box::new(TransportError::new(
            TransportErrorStage::Tls,
            TransportFailureScope::EgressPath,
            RetrySafety::DefinitelyNotSent,
            "egress denied",
        )),
        candidate: Box::new(failed.clone()),
        bound: false,
    };
    let mut exclusions = CandidateExclusions::default();

    exclude_failed_path(&mut exclusions, &failure, RetryExclusion::EgressPath);

    assert!(!exclusions.allows(&failed));
    assert!(exclusions.allows(&other_proxy));
    assert!(exclusions.allows(&other_endpoint));
    assert!(exclusions.allows(&new_proxy_generation));
}
