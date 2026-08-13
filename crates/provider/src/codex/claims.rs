use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use serde::Deserialize;

#[derive(Default)]
pub(super) struct DecodedClaims {
    pub(super) account_id: Option<String>,
    pub(super) email: Option<String>,
    pub(super) member_id: Option<String>,
    pub(super) plan: Option<String>,
}

pub(super) fn decode(token: Option<&str>) -> DecodedClaims {
    let Some(payload) = token.and_then(|token| token.split('.').nth(1)) else {
        return DecodedClaims::default();
    };
    let Ok(bytes) = URL_SAFE_NO_PAD.decode(payload) else {
        return DecodedClaims::default();
    };
    let Ok(claims) = serde_json::from_slice::<Claims>(&bytes) else {
        return DecodedClaims::default();
    };
    let (account_id, member_id, plan) = claims
        .auth
        .map(|auth| {
            (
                auth.chatgpt_account_id,
                auth.chatgpt_user_id.or(auth.user_id),
                auth.chatgpt_plan_type,
            )
        })
        .unwrap_or_default();
    DecodedClaims {
        account_id,
        email: claims
            .email
            .or_else(|| claims.profile.and_then(|profile| profile.email)),
        member_id,
        plan,
    }
}

#[derive(Deserialize)]
struct Claims {
    email: Option<String>,
    profile: Option<ProfileClaims>,
    #[serde(rename = "https://api.openai.com/auth")]
    auth: Option<AuthClaims>,
}

#[derive(Deserialize)]
struct ProfileClaims {
    email: Option<String>,
}

#[derive(Deserialize)]
struct AuthClaims {
    chatgpt_account_id: Option<String>,
    chatgpt_plan_type: Option<String>,
    chatgpt_user_id: Option<String>,
    user_id: Option<String>,
}
