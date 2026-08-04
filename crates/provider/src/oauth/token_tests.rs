use std::sync::Arc;

use serde_json::json;

use crate::{ClaudeDriver, CodexDriver, GrokDriver, api::ProviderRegistry};

#[test]
fn registered_oauth_drivers_require_a_positive_bounded_expires_in() {
    let mut registry = ProviderRegistry::new();
    registry
        .register(Arc::new(CodexDriver::new()))
        .expect("Codex");
    registry
        .register(Arc::new(ClaudeDriver::new()))
        .expect("Claude");
    registry
        .register(Arc::new(GrokDriver::new()))
        .expect("Grok");

    for (kind, driver) in registry.iter() {
        for expires_in in [None, Some(0), Some(-1), Some(i64::MAX)] {
            let mut response = json!({"access_token":"access-secret"});
            if let Some(expires_in) = expires_in {
                response["expires_in"] = expires_in.into();
            }
            assert!(
                driver
                    .parse_oauth_token_response(response.to_string().as_bytes())
                    .is_err(),
                "{kind:?} accepted expires_in {expires_in:?}"
            );
        }
        assert!(
            driver
                .parse_oauth_token_response(
                    br#"{"access_token":"access-secret","expires_in":3600}"#,
                )
                .is_ok(),
            "{kind:?} rejected a valid expires_in"
        );
    }
}
