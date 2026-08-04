//! Grok Build `/user?include=subscription` account diagnostics.

use serde::{Deserialize, Deserializer};

use crate::{OAuthQuotaSupplement, ProviderError};

pub(crate) fn parse_subscription(body: &[u8]) -> Result<OAuthQuotaSupplement, ProviderError> {
    let payload = serde_json::from_slice::<SubscriptionPayload>(body)
        .map_err(|_| invalid_response("Grok subscription response is invalid"))?;
    let subscription_tier = match payload.subscription_tier {
        SubscriptionTierField::Missing => None,
        SubscriptionTierField::Present(None) => Some("Free".into()),
        SubscriptionTierField::Present(Some(value)) => non_empty(Some(value)),
    };
    Ok(OAuthQuotaSupplement {
        subscription_tier,
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
    #[serde(default)]
    subscription_tier: SubscriptionTierField,
    #[serde(default)]
    user_blocked_reason: Option<String>,
    #[serde(default)]
    team_blocked_reasons: Option<Vec<String>>,
}

#[derive(Default)]
enum SubscriptionTierField {
    #[default]
    Missing,
    Present(Option<String>),
}

impl<'de> Deserialize<'de> for SubscriptionTierField {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Option::<String>::deserialize(deserializer).map(Self::Present)
    }
}

fn non_empty(value: Option<String>) -> Option<String> {
    value
        .map(|value| value.trim().to_owned())
        .filter(|value| !value.is_empty())
}

fn invalid_response(message: &'static str) -> ProviderError {
    ProviderError::InvalidResponse(message.into())
}
