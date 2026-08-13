use any2api_domain::ProviderKind;
use any2api_provider::api::{OAuthTokenMaterial, ProviderDriver};
use base64::Engine as _;

use super::{OAuthImportIdentity, OAuthImportIdentityIndex};

fn token(
    provider: ProviderKind,
    account_id: Option<&str>,
    email: Option<&str>,
) -> OAuthTokenMaterial {
    token_with_access(provider, "access-token", account_id, email)
}

fn token_with_access(
    provider: ProviderKind,
    access_token: &str,
    account_id: Option<&str>,
    email: Option<&str>,
) -> OAuthTokenMaterial {
    OAuthTokenMaterial::new(
        provider,
        access_token.into(),
        None,
        None,
        None,
        account_id.map(str::to_owned),
        email.map(str::to_owned),
    )
    .expect("token")
}

fn identity(driver: &dyn ProviderDriver, token: &OAuthTokenMaterial) -> OAuthImportIdentity {
    OAuthImportIdentity::from_token(driver, token)
}

fn provider_driver(
    capabilities: &crate::configuration::ConfigurationCapabilities,
    provider: ProviderKind,
) -> &dyn ProviderDriver {
    capabilities
        .provider_registry()
        .get(provider)
        .expect("registered Provider driver")
        .as_ref()
}

fn codex_token(workspace: &str, member: &str, email: &str) -> OAuthTokenMaterial {
    let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "email": email,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": workspace,
                "chatgpt_user_id": member,
            }
        })
        .to_string(),
    );
    OAuthTokenMaterial::new(
        ProviderKind::Codex,
        format!("header.{payload}.signature"),
        None,
        Some(format!("header.{payload}.signature")),
        None,
        Some(workspace.to_owned()),
        Some(email.to_owned()),
    )
    .expect("Codex token")
}

#[test]
fn codex_workspace_members_are_distinct_but_reauthorization_is_stable() {
    let capabilities = crate::test_support::configuration_capabilities();
    let driver = provider_driver(capabilities.as_ref(), ProviderKind::Codex);
    let first = codex_token("workspace-a", "member-a", "first@example.com");
    let renamed = codex_token("workspace-a", "member-a", "renamed@example.com");
    let second = codex_token("workspace-a", "member-b", "second@example.com");
    let other_workspace = codex_token("workspace-b", "member-a", "first@example.com");

    assert!(identity(driver, &first).stable_matches(&identity(driver, &renamed)));
    assert!(!identity(driver, &first).stable_matches(&identity(driver, &second)));
    assert!(!identity(driver, &first).stable_matches(&identity(driver, &other_workspace)));
}

#[test]
fn provider_account_identity_remains_isolated_for_non_codex_drivers() {
    let capabilities = crate::test_support::configuration_capabilities();
    let claude = provider_driver(capabilities.as_ref(), ProviderKind::Claude);
    let grok = provider_driver(capabilities.as_ref(), ProviderKind::Grok);
    let first = token(
        ProviderKind::Claude,
        Some("account-a"),
        Some("person@example.com"),
    );
    let renamed = token(
        ProviderKind::Claude,
        Some("account-a"),
        Some("renamed@example.com"),
    );
    let other_provider = token(
        ProviderKind::Grok,
        Some("account-a"),
        Some("person@example.com"),
    );

    assert!(identity(claude, &first).stable_matches(&identity(claude, &renamed)));
    assert!(!identity(claude, &first).stable_matches(&identity(grok, &other_provider)));
}

#[test]
fn email_only_matches_another_token_without_account_id() {
    let capabilities = crate::test_support::configuration_capabilities();
    let driver = provider_driver(capabilities.as_ref(), ProviderKind::Claude);
    let account_identity = identity(
        driver,
        &token(ProviderKind::Claude, None, Some(" Person@Example.COM ")),
    );

    assert!(account_identity.stable_matches(&identity(
        driver,
        &token(ProviderKind::Claude, None, Some("person@example.com")),
    )));
    assert!(!account_identity.stable_matches(&identity(
        driver,
        &token(
            ProviderKind::Claude,
            Some("stable-id"),
            Some("person@example.com")
        ),
    )));
    assert!(
        OAuthImportIdentity::from_token(driver, &token(ProviderKind::Claude, None, None),)
            .keys
            .iter()
            .all(|key| !matches!(key, super::OAuthImportIdentityKey::Stable(_)))
    );
}

#[test]
fn import_identity_rejects_stable_or_exact_secret_duplicates() {
    let stable = codex_token("account-a", "member-a", "person@example.com");
    let same_stable = codex_token("account-a", "member-a", "renamed@example.com");
    let mut index = OAuthImportIdentityIndex::default();
    let capabilities = crate::test_support::configuration_capabilities();
    let codex = provider_driver(capabilities.as_ref(), ProviderKind::Codex);
    assert!(index.insert_new(&identity(codex, &stable)));
    assert!(!index.insert_new(&identity(codex, &same_stable)));

    let claude = provider_driver(capabilities.as_ref(), ProviderKind::Claude);
    let first = OAuthTokenMaterial::new(
        ProviderKind::Claude,
        "same-access".into(),
        Some("same-refresh".into()),
        None,
        None,
        None,
        None,
    )
    .expect("first token");
    let same_refresh = OAuthTokenMaterial::new(
        ProviderKind::Claude,
        "other-access".into(),
        Some("same-refresh".into()),
        None,
        None,
        None,
        None,
    )
    .expect("second token");
    assert!(index.insert_new(&identity(claude, &first)));
    assert!(!index.insert_new(&identity(claude, &same_refresh)));
}

#[test]
fn import_identity_keeps_unproven_accounts_distinct() {
    let first = token_with_access(
        ProviderKind::Claude,
        "access-a",
        Some("account-a"),
        Some("shared@example.com"),
    );
    let second = token_with_access(
        ProviderKind::Claude,
        "access-b",
        Some("account-b"),
        Some("shared@example.com"),
    );
    let third = token_with_access(
        ProviderKind::Claude,
        "access-c",
        None,
        Some("shared@example.com"),
    );
    let mut index = OAuthImportIdentityIndex::default();
    let capabilities = crate::test_support::configuration_capabilities();
    let driver = provider_driver(capabilities.as_ref(), ProviderKind::Claude);

    assert!(index.insert_new(&identity(driver, &first)));
    assert!(index.insert_new(&identity(driver, &second)));
    assert!(index.insert_new(&identity(driver, &third)));
}

#[test]
fn login_identity_can_match_exact_token_without_stable_identity() {
    let existing = OAuthTokenMaterial::new(
        ProviderKind::Claude,
        "access-a".into(),
        Some("refresh-a".into()),
        Some("id-a".into()),
        None,
        Some("account-a".into()),
        Some("person@example.com".into()),
    )
    .expect("existing token");
    let capabilities = crate::test_support::configuration_capabilities();
    let driver = provider_driver(capabilities.as_ref(), ProviderKind::Claude);
    let incoming = [
        OAuthTokenMaterial::new(
            ProviderKind::Claude,
            "access-a".into(),
            None,
            None,
            None,
            None,
            None,
        )
        .expect("same access token"),
        OAuthTokenMaterial::new(
            ProviderKind::Claude,
            "different-access".into(),
            Some("refresh-a".into()),
            None,
            None,
            None,
            None,
        )
        .expect("same refresh token"),
        OAuthTokenMaterial::new(
            ProviderKind::Claude,
            "different-access".into(),
            Some("different-refresh".into()),
            Some("id-a".into()),
            None,
            None,
            None,
        )
        .expect("same ID token"),
    ];

    for incoming in incoming {
        let incoming_identity = identity(driver, &incoming);
        let existing_identity = identity(driver, &existing);
        assert!(!incoming_identity.stable_matches(&existing_identity));
        assert!(incoming_identity.exact_token_matches_identity(&existing_identity));
    }
    let different_provider = identity(
        provider_driver(capabilities.as_ref(), ProviderKind::Grok),
        &OAuthTokenMaterial::new(
            ProviderKind::Grok,
            "access-a".into(),
            Some("refresh-a".into()),
            Some("id-a".into()),
            None,
            None,
            None,
        )
        .expect("different provider token"),
    );
    assert!(!different_provider.exact_token_matches_identity(&identity(driver, &existing)));
}
