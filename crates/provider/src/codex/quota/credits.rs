//! Purchased Credits and workspace access fields from the Codex quota response.

use serde::Deserialize;

use crate::{
    ProviderError,
    oauth::quota::{OAuthQuotaCredits, OAuthQuotaReachedType},
};

const MAX_CREDIT_BALANCE_BYTES: usize = 128;

#[derive(Deserialize)]
pub(super) struct CreditsPayload {
    pub(super) has_credits: bool,
    pub(super) unlimited: bool,
    pub(super) balance: Option<String>,
}

#[derive(Deserialize)]
pub(super) struct SpendControlPayload {
    pub(super) reached: bool,
}

#[derive(Deserialize)]
pub(super) struct ReachedTypePayload {
    #[serde(rename = "type")]
    pub(super) kind: String,
}

pub(super) fn parse_credits(value: CreditsPayload) -> Result<OAuthQuotaCredits, ProviderError> {
    let balance = value
        .balance
        .map(|balance| {
            let balance = balance.trim();
            if balance.is_empty() {
                return Ok(None);
            }
            if balance.len() > MAX_CREDIT_BALANCE_BYTES || !is_non_negative_decimal(balance) {
                return Err(invalid_response("Codex Credits balance is invalid"));
            }
            Ok(Some(balance.to_owned()))
        })
        .transpose()?
        .flatten();
    Ok(OAuthQuotaCredits {
        has_credits: value.has_credits,
        unlimited: value.unlimited,
        balance,
    })
}

pub(super) fn parse_reached_type(value: &str) -> Option<OAuthQuotaReachedType> {
    match value {
        "rate_limit_reached" => Some(OAuthQuotaReachedType::RateLimitReached),
        "workspace_owner_credits_depleted" => {
            Some(OAuthQuotaReachedType::WorkspaceOwnerCreditsDepleted)
        }
        "workspace_member_credits_depleted" => {
            Some(OAuthQuotaReachedType::WorkspaceMemberCreditsDepleted)
        }
        "workspace_owner_usage_limit_reached" => {
            Some(OAuthQuotaReachedType::WorkspaceOwnerUsageLimitReached)
        }
        "workspace_member_usage_limit_reached" => {
            Some(OAuthQuotaReachedType::WorkspaceMemberUsageLimitReached)
        }
        _ => None,
    }
}

fn is_non_negative_decimal(value: &str) -> bool {
    let mut decimal_seen = false;
    let mut digit_seen = false;
    for byte in value.bytes() {
        match byte {
            b'0'..=b'9' => digit_seen = true,
            b'.' if !decimal_seen => decimal_seen = true,
            _ => return false,
        }
    }
    digit_seen && !value.ends_with('.')
}

fn invalid_response(message: &'static str) -> ProviderError {
    ProviderError::InvalidResponse(message.into())
}
