use crate::{
    OAuthAccount, OAuthAccountConfiguration, OAuthAccountDraft, OAuthAccountId,
    OAuthAccountValidationError, ProviderKind, ProxyConfiguration, ProxyProfile, ProxyProfileId,
    RequestsPerMinute,
};

fn account(provider: ProviderKind, label: &str) -> OAuthAccount {
    OAuthAccount::create(
        OAuthAccountId::new(),
        provider,
        OAuthAccountDraft::new(
            label,
            Some(RequestsPerMinute::new(60).expect("valid RPM")),
            true,
        )
        .expect("valid draft"),
        Some("owner@example.com".into()),
        Some(100),
        vec!["model".into()],
    )
    .expect("valid account")
}

fn proxies() -> ProxyConfiguration {
    ProxyConfiguration::new(vec![ProxyProfile::direct()], ProxyProfileId::DIRECT)
        .expect("valid proxies")
}

#[test]
fn refresh_changes_only_authentication_version_and_safe_metadata() {
    let account = account(ProviderKind::Codex, "Primary");

    let refreshed = account
        .refreshed(Some("new@example.com".into()), Some(200))
        .expect("refresh");

    assert_eq!(refreshed.token_version(), 2);
    assert_eq!(refreshed.account_generation(), 1);
    assert_eq!(refreshed.config_version(), 1);
    assert_eq!(refreshed.models(), account.models());
    assert_eq!(refreshed.safe_account_email(), Some("new@example.com"));
}

#[test]
fn reauthorization_preserves_local_settings_and_versions_model_changes() {
    let account = account(ProviderKind::Codex, "Primary");
    let reauthorized = account
        .reauthorized(
            Some("new@example.com".into()),
            Some(200),
            vec!["other-model".into()],
        )
        .expect("reauthorize");

    assert_eq!(reauthorized.id(), account.id());
    assert_eq!(reauthorized.label(), account.label());
    assert_eq!(
        reauthorized.requests_per_minute(),
        account.requests_per_minute()
    );
    assert_eq!(reauthorized.enabled(), account.enabled());
    assert_eq!(reauthorized.token_version(), 2);
    assert_eq!(reauthorized.account_generation(), 1);
    assert_eq!(reauthorized.config_version(), 2);
    assert_eq!(reauthorized.models()[0].as_str(), "other-model");
}

#[test]
fn reenable_changes_account_generation_and_refresh_preserves_it() {
    let account = account(ProviderKind::Codex, "Primary");
    let disabled = account
        .updated(OAuthAccountDraft::new("Primary", None, false).expect("disabled draft"))
        .expect("disable account");
    let enabled = disabled
        .updated(OAuthAccountDraft::new("Primary", None, true).expect("enabled draft"))
        .expect("reenable account");
    let refreshed = enabled.refreshed(None, Some(200)).expect("refresh account");

    assert_eq!(disabled.account_generation(), 1);
    assert_eq!(enabled.account_generation(), 2);
    assert_eq!(refreshed.account_generation(), 2);
    assert_eq!(refreshed.token_version(), 2);
}

#[test]
fn labels_are_unique_per_provider() {
    OAuthAccountConfiguration::new(
        vec![
            account(ProviderKind::Codex, "Primary"),
            account(ProviderKind::Claude, "Primary"),
        ],
        &proxies(),
    )
    .expect("labels may repeat across providers");

    let error = OAuthAccountConfiguration::new(
        vec![
            account(ProviderKind::Codex, "Primary"),
            account(ProviderKind::Codex, "Primary"),
        ],
        &proxies(),
    )
    .expect_err("same-provider labels conflict");
    assert_eq!(error, OAuthAccountValidationError::DuplicateLabel);
}

#[test]
fn grok_can_be_constructed_as_an_independent_oauth_account() {
    let account = account(ProviderKind::Grok, "Grok");

    assert_eq!(account.provider_kind(), ProviderKind::Grok);
    assert_eq!(account.proxy_profile_id(), ProxyProfileId::DIRECT);
}
