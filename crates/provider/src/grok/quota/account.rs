//! Grok Build `/user?include=subscription` account diagnostics.

use serde::Deserialize;

use crate::{OAuthQuotaSupplement, ProviderError};

pub(crate) fn parse_subscription(body: &[u8]) -> Result<OAuthQuotaSupplement, ProviderError> {
    let payload = serde_json::from_slice::<SubscriptionPayload>(body)
        .map_err(|_| invalid_response("Grok subscription response is invalid"))?;
    Ok(OAuthQuotaSupplement {
        subscription_tier: Some(
            non_empty(payload.subscription_tier).unwrap_or_else(|| "Free".into()),
        ),
        user_blocked_reason: non_empty(payload.user_blocked_reason),
        team_blocked_reasons: payload
            .team_blocked_reasons
            .unwrap_or_default()
            .into_iter()
            .filter_map(|value| non_empty(Some(value)))
            .collect(),
    })
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct SubscriptionPayload {
    #[serde(default, alias = "subscription_tier")]
    subscription_tier: Option<String>,
    #[serde(default, alias = "user_blocked_reason")]
    user_blocked_reason: Option<String>,
    #[serde(default, alias = "team_blocked_reasons")]
    team_blocked_reasons: Option<Vec<String>>,
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn invalid_response(message: &'static str) -> ProviderError {
    ProviderError::InvalidResponse(message.into())
}
