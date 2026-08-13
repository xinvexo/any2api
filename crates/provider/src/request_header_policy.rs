use http::{HeaderMap, HeaderName};

use crate::header_policy::project_selected;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RequestHeaderOwnership {
    Replayable,
    /// Session, device, request-correlation and trace values. Replayed across
    /// credential switches in balanced (affinity-off) mode because upstream
    /// prompt caches route by them; dropped on switch in sticky mode.
    SessionScoped,
    CredentialOwned,
    BoundTurnState,
}

pub(crate) struct RequestHeaderRule {
    name: HeaderName,
    ownership: RequestHeaderOwnership,
}

#[derive(Clone, Copy)]
pub(crate) struct RequestHeaderPrefixRule {
    prefix: &'static str,
    ownership: RequestHeaderOwnership,
}

impl RequestHeaderPrefixRule {
    pub(crate) const fn new(prefix: &'static str, ownership: RequestHeaderOwnership) -> Self {
        Self { prefix, ownership }
    }
}

pub(crate) fn request_header_rules(
    values: &[(&str, RequestHeaderOwnership)],
) -> Vec<RequestHeaderRule> {
    let mut rules = Vec::with_capacity(values.len());
    for (value, ownership) in values {
        let Ok(name) = HeaderName::from_bytes(value.as_bytes()) else {
            continue;
        };
        if rules
            .iter()
            .any(|rule: &RequestHeaderRule| rule.name == name)
        {
            continue;
        }
        rules.push(RequestHeaderRule {
            name,
            ownership: *ownership,
        });
    }
    rules
}

pub(crate) fn project_request(
    source: &HeaderMap,
    exact: &[RequestHeaderRule],
    prefixes: &[RequestHeaderPrefixRule],
    allow_credential_owned: bool,
    allow_turn_state: bool,
    allow_session_replay: bool,
) -> HeaderMap {
    let selected_exact = exact
        .iter()
        .filter(|rule| {
            rule.ownership.allowed(
                allow_credential_owned,
                allow_turn_state,
                allow_session_replay,
            )
        })
        .map(|rule| &rule.name);
    let known_exact = exact.iter().map(|rule| &rule.name);
    let selected_prefixes = prefixes
        .iter()
        .filter(|rule| {
            rule.ownership.allowed(
                allow_credential_owned,
                allow_turn_state,
                allow_session_replay,
            )
        })
        .map(|rule| rule.prefix);
    project_selected(source, selected_exact, known_exact, selected_prefixes)
}

impl RequestHeaderOwnership {
    fn allowed(
        self,
        allow_credential_owned: bool,
        allow_turn_state: bool,
        allow_session_replay: bool,
    ) -> bool {
        match self {
            Self::Replayable => true,
            Self::SessionScoped => allow_session_replay,
            Self::CredentialOwned => allow_credential_owned,
            Self::BoundTurnState => allow_credential_owned && allow_turn_state,
        }
    }
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue};

    use super::{
        RequestHeaderOwnership, RequestHeaderPrefixRule, project_request, request_header_rules,
    };

    #[test]
    fn ownership_filters_exact_and_prefix_rules_without_reintroducing_known_names() {
        let exact = request_header_rules(&[
            ("x-replayable", RequestHeaderOwnership::Replayable),
            ("x-session", RequestHeaderOwnership::SessionScoped),
            ("x-owned", RequestHeaderOwnership::CredentialOwned),
            (
                "x-owned-prefix-exact",
                RequestHeaderOwnership::CredentialOwned,
            ),
            ("x-turn-state", RequestHeaderOwnership::BoundTurnState),
        ]);
        let prefixes = [RequestHeaderPrefixRule::new(
            "x-owned-prefix-",
            RequestHeaderOwnership::Replayable,
        )];
        let mut source = HeaderMap::new();
        for name in [
            "x-replayable",
            "x-session",
            "x-owned",
            "x-owned-prefix-exact",
            "x-owned-prefix-future",
            "x-turn-state",
        ] {
            source.insert(name, HeaderValue::from_static("value"));
        }

        let switched = project_request(&source, &exact, &prefixes, false, false, true);

        assert_eq!(switched["x-replayable"], "value");
        assert_eq!(switched["x-session"], "value");
        assert_eq!(switched["x-owned-prefix-future"], "value");
        assert!(!switched.contains_key("x-owned"));
        assert!(!switched.contains_key("x-owned-prefix-exact"));
        assert!(!switched.contains_key("x-turn-state"));

        let owner_without_binding = project_request(&source, &exact, &prefixes, true, false, true);
        assert_eq!(owner_without_binding["x-owned"], "value");
        assert!(!owner_without_binding.contains_key("x-turn-state"));

        let bound_owner = project_request(&source, &exact, &prefixes, true, true, true);
        assert_eq!(bound_owner["x-turn-state"], "value");

        let sticky_switched = project_request(&source, &exact, &prefixes, false, false, false);
        assert_eq!(sticky_switched["x-replayable"], "value");
        assert!(!sticky_switched.contains_key("x-session"));
        assert!(!sticky_switched.contains_key("x-owned"));
    }
}
