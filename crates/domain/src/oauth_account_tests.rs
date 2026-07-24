use crate::{
    MaxConcurrency, OAuthAccount, OAuthAccountConfiguration, OAuthAccountDraft, OAuthAccountId,
    OAuthAccountValidationError, ProviderKind, ProxyConfiguration, ProxyProfile, ProxyProfileId,
};

fn account(provider: ProviderKind, label: &str) -> OAuthAccount {
    OAuthAccount::create(
        OAuthAccountId::new(),
        provider,
        OAuthAccountDraft::new(label, MaxConcurrency::new(1).expect("valid limit"), true)
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
fn refresh_changes_only_auth_generation_and_safe_metadata() {
    let account = account(ProviderKind::Codex, "Primary");

    let refreshed = account
        .refreshed(Some("new@example.com".into()), Some(200))
        .expect("refresh");

    assert_eq!(refreshed.token_version(), 2);
    assert_eq!(refreshed.account_generation(), 2);
    assert_eq!(refreshed.config_version(), 1);
    assert_eq!(refreshed.models(), account.models());
    assert_eq!(refreshed.safe_account_email(), Some("new@example.com"));
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
