use std::str::FromStr;

use any2api_domain::{PublicModelName, RequestLogFilter, RequestLogOutcomeFilter};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::log_cursor::{LogBatchRequest, validate_request_log_batch};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct RequestLogListQuery {
    cursor: Option<String>,
    outcome: Option<String>,
    public_model: Option<String>,
    gateway_api_key_id: Option<String>,
}

pub(super) struct ValidatedRequestLogListQuery {
    pub(super) batch: LogBatchRequest,
    pub(super) filter: RequestLogFilter,
    pub(super) filter_fingerprint: String,
}

impl RequestLogListQuery {
    pub(super) fn validate(self) -> Option<ValidatedRequestLogListQuery> {
        let outcome = match self.outcome.as_deref() {
            Some(value) => Some(RequestLogOutcomeFilter::parse(value)?),
            None => None,
        };
        let public_model = self
            .public_model
            .map(PublicModelName::new)
            .transpose()
            .ok()?;
        let gateway_api_key_id = parse_id(self.gateway_api_key_id)?;
        let filter = RequestLogFilter::new(outcome, public_model, gateway_api_key_id);
        let filter_fingerprint = filter_fingerprint(&filter);
        let batch = validate_request_log_batch(self.cursor, &filter_fingerprint)?;
        Some(ValidatedRequestLogListQuery {
            batch,
            filter,
            filter_fingerprint,
        })
    }
}

fn parse_id<T: FromStr>(value: Option<String>) -> Option<Option<T>> {
    match value {
        Some(value) => Some(Some(value.parse().ok()?)),
        None => Some(None),
    }
}

fn filter_fingerprint(filter: &RequestLogFilter) -> String {
    #[derive(Serialize)]
    struct CanonicalFilter<'a> {
        outcome: Option<&'static str>,
        public_model: Option<&'a str>,
        gateway_api_key_id: Option<String>,
    }

    let canonical = CanonicalFilter {
        outcome: filter.outcome().map(RequestLogOutcomeFilter::as_str),
        public_model: filter.public_model().map(PublicModelName::as_str),
        gateway_api_key_id: filter.gateway_api_key_id().map(|id| id.to_string()),
    };
    let canonical = serde_json::to_vec(&canonical).expect("canonical filter is serializable");
    URL_SAFE_NO_PAD.encode(Sha256::digest(canonical))
}

#[cfg(test)]
mod tests {
    use any2api_domain::GatewayApiKeyId;

    use super::*;

    fn query() -> RequestLogListQuery {
        RequestLogListQuery {
            cursor: None,
            outcome: Some("cancelled".into()),
            public_model: Some("gpt-test".into()),
            gateway_api_key_id: Some(GatewayApiKeyId::new().to_string()),
        }
    }

    #[test]
    fn validates_exact_filters_and_rejects_unknown_outcomes() {
        let validated = query().validate().expect("valid filters");
        assert_eq!(
            validated.filter.outcome(),
            Some(RequestLogOutcomeFilter::Cancelled)
        );
        assert!(validated.batch.cursor.is_none());
        let mut obsolete_outcome = query();
        obsolete_outcome.outcome = Some("failure".into());
        assert!(obsolete_outcome.validate().is_none());
    }

    #[test]
    fn rejects_removed_filter_fields() {
        assert!(
            serde_json::from_value::<RequestLogListQuery>(serde_json::json!({
                "operation": "responses"
            }))
            .is_err()
        );
        assert!(
            serde_json::from_value::<RequestLogListQuery>(serde_json::json!({
                "credential_id": GatewayApiKeyId::new().to_string()
            }))
            .is_err()
        );
    }
}
