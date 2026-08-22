use std::{str::FromStr, time::Duration};

use any2api_domain::ProviderKind;
use any2api_provider::api::{OAuthGrant, OAuthRequestPlan, OAuthTokenMaterial, ProviderRegistry};
use any2api_transport::api::{
    EndpointNetworkPolicy, TransportIsolationKey, TransportRequest, TransportTrafficClass,
};
use bytes::Bytes;
use http::Uri;

use super::types::{Surface, SurfaceCase};

pub(super) fn append_control_plane_cases(
    cases: &mut Vec<SurfaceCase>,
    providers: &ProviderRegistry,
    tokens: &[(ProviderKind, OAuthTokenMaterial)],
) {
    for (kind, token) in tokens {
        let driver = providers.get(*kind).expect("OAuth Provider driver");
        match kind {
            ProviderKind::Codex | ProviderKind::Claude => {
                push_plan(
                    cases,
                    format!("token.{}.authorization_code", kind.as_str()),
                    *kind,
                    Surface::OAuthToken,
                    "oauth_client_grant",
                    driver
                        .oauth_token_request(
                            OAuthGrant::AuthorizationCode,
                            "fixture-authorization-code",
                            Some("fixture-state"),
                            Some("fixture-code-verifier"),
                        )
                        .expect("authorization-code plan"),
                );
                push_plan(
                    cases,
                    format!("token.{}.refresh", kind.as_str()),
                    *kind,
                    Surface::OAuthToken,
                    "oauth_client_grant",
                    driver
                        .oauth_token_request(
                            OAuthGrant::RefreshToken,
                            "fixture-refresh-token",
                            None,
                            None,
                        )
                        .expect("refresh-token plan"),
                );
            }
            ProviderKind::Grok => {
                push_plan(
                    cases,
                    "token.grok.device_authorization".into(),
                    *kind,
                    Surface::OAuthToken,
                    "oauth_client_grant",
                    driver
                        .oauth_device_authorization_request()
                        .expect("device authorization plan"),
                );
                push_plan(
                    cases,
                    "token.grok.device_token".into(),
                    *kind,
                    Surface::OAuthToken,
                    "oauth_client_grant",
                    driver
                        .oauth_device_token_request("fixture-device-code")
                        .expect("device token plan"),
                );
                push_plan(
                    cases,
                    "token.grok.refresh".into(),
                    *kind,
                    Surface::OAuthToken,
                    "oauth_client_grant",
                    driver
                        .oauth_token_request(
                            OAuthGrant::RefreshToken,
                            "fixture-refresh-token",
                            None,
                            None,
                        )
                        .expect("Grok refresh plan"),
                );
            }
            ProviderKind::OpenAi | ProviderKind::Kimi => {
                unreachable!("API-key-only Provider has no OAuth token")
            }
        }

        let query = driver
            .oauth_quota_query_plan(token)
            .expect("quota plan")
            .expect("OAuth Provider quota support");
        let (usage, supplement, reset_credits) = query.into_parts();
        push_plan(
            cases,
            format!("quota.{}.usage", kind.as_str()),
            *kind,
            Surface::OAuthQuota,
            "oauth_access_token",
            usage,
        );
        if let Some(plan) = supplement {
            push_plan(
                cases,
                format!("quota.{}.supplement", kind.as_str()),
                *kind,
                Surface::OAuthQuota,
                "oauth_access_token",
                plan,
            );
        }
        if let Some(plan) = reset_credits {
            push_plan(
                cases,
                format!("quota.{}.reset_credits", kind.as_str()),
                *kind,
                Surface::OAuthQuota,
                "oauth_access_token",
                plan,
            );
        }
        if let Some(plan) = driver
            .oauth_quota_reset_plan(token, "fixture-redeem-request-id")
            .expect("quota reset plan")
        {
            push_plan(
                cases,
                format!("quota.{}.reset", kind.as_str()),
                *kind,
                Surface::OAuthQuota,
                "oauth_access_token",
                plan,
            );
        }
    }
}

pub(super) fn oauth_tokens() -> Vec<(ProviderKind, OAuthTokenMaterial)> {
    [
        ProviderKind::Codex,
        ProviderKind::Claude,
        ProviderKind::Grok,
    ]
    .into_iter()
    .map(|kind| {
        let account_id = match kind {
            ProviderKind::Codex | ProviderKind::Grok => {
                Some(format!("fixture-{}-account", kind.as_str()))
            }
            ProviderKind::Claude => None,
            ProviderKind::OpenAi | ProviderKind::Kimi => {
                unreachable!("API-key-only Provider has no OAuth token")
            }
        };
        let token = OAuthTokenMaterial::new(
            kind,
            format!("fixture-{}-oauth-access", kind.as_str()),
            Some("fixture-refresh-token".into()),
            None,
            None,
            account_id,
            Some(format!("{}@example.test", kind.as_str())),
        )
        .expect("fixture OAuth token");
        (kind, token)
    })
    .collect()
}

fn push_plan(
    cases: &mut Vec<SurfaceCase>,
    name: String,
    provider: ProviderKind,
    surface: Surface,
    auth_class: &'static str,
    plan: OAuthRequestPlan,
) {
    let traffic_class = match surface {
        Surface::OAuthToken => TransportTrafficClass::OAuthToken,
        Surface::OAuthQuota => TransportTrafficClass::OAuthQuota,
        Surface::DataDirect | Surface::DataBridge => unreachable!("control-plane plan"),
    };
    let target = plan.url.to_string();
    cases.push(SurfaceCase {
        name,
        provider,
        surface,
        auth_class,
        target,
        request: TransportRequest {
            method: plan.method,
            uri: Uri::from_str(plan.url.as_str()).expect("control-plane endpoint URI"),
            headers: plan.headers,
            body: Bytes::from(plan.body),
            isolation: TransportIsolationKey::ephemeral(traffic_class),
            network_policy: EndpointNetworkPolicy::new(),
            read_timeout: Duration::from_secs(15),
        },
    });
}
