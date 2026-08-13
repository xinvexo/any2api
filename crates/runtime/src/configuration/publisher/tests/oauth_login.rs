use any2api_domain::{OAuthAccountDraft, OAuthAccountId, ProviderKind};
use any2api_provider::api::{OAuthTokenMaterial, encode_oauth_account_document};
use any2api_storage::api::{ConfigurationRepository, OAuthAccountDocument};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};

use super::{TestContext, oauth_account_draft};
use crate::configuration::{ConfigPublishError, OAuthAccountActivation};

#[tokio::test]
async fn exact_token_login_without_identity_reauthorizes_existing_account() {
    let context = TestContext::new().await;
    let existing_id = OAuthAccountId::new();
    let existing = token(
        "old-access",
        Some("shared-refresh"),
        Some("old-id-token"),
        Some("account-a"),
        Some("person@example.com"),
    );
    context
        .publisher
        .activate_oauth_account(
            existing_id,
            ProviderKind::Claude,
            OAuthAccountDraft::new("Existing OAuth", None, false).expect("disabled OAuth draft"),
            Some("person@example.com".to_owned()),
            Some(1_800_000_000),
            Vec::new(),
            document(&existing),
        )
        .await
        .expect("existing account");
    let incoming = token("new-access", Some("shared-refresh"), None, None, None);

    let (published, account_id) = context
        .publisher
        .activate_oauth_login(activation(OAuthAccountId::new(), incoming))
        .await
        .expect("reauthorize exact token match");
    let account = published
        .oauth_accounts()
        .get(existing_id)
        .expect("existing account remains");
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");
    let current_token = published
        .oauth_token_material(existing_id)
        .expect("published token");

    assert_eq!(account_id, existing_id);
    assert_eq!(published.oauth_accounts().accounts().len(), 1);
    assert_eq!(stored.oauth_accounts().accounts().len(), 1);
    assert_eq!(published.revision(), stored.revision());
    assert_eq!(published.revision().get(), 3);
    assert_eq!(context.runtime.scheduler_epoch(), 2);
    assert_eq!(account.label(), "Existing OAuth");
    assert!(!account.enabled());
    assert_eq!(account.token_version(), 2);
    assert_eq!(account.safe_account_email(), Some("person@example.com"));
    assert_eq!(current_token.access_token(), "new-access");
    assert_eq!(current_token.refresh_token(), Some("shared-refresh"));
    assert_eq!(current_token.account_id(), Some("account-a"));
    assert_eq!(current_token.email(), Some("person@example.com"));
}

#[tokio::test]
async fn stable_and_exact_token_conflict_does_not_publish() {
    let context = TestContext::new().await;
    let stable_id = OAuthAccountId::new();
    let exact_id = OAuthAccountId::new();
    let stable = token(
        "stable-access",
        Some("stable-refresh"),
        None,
        Some("account-a"),
        Some("first@example.com"),
    );
    let exact = token(
        "exact-access",
        Some("shared-refresh"),
        None,
        Some("account-b"),
        Some("second@example.com"),
    );
    let first = context
        .publisher
        .activate_oauth_account(
            stable_id,
            ProviderKind::Claude,
            oauth_account_draft("Stable OAuth"),
            stable.email().map(str::to_owned),
            stable.expires_at(),
            Vec::new(),
            document(&stable),
        )
        .await
        .expect("stable account");
    context
        .publisher
        .activate_oauth_account(
            exact_id,
            ProviderKind::Claude,
            oauth_account_draft("Exact OAuth"),
            exact.email().map(str::to_owned),
            exact.expires_at(),
            Vec::new(),
            document(&exact),
        )
        .await
        .expect("exact account");
    let revision = context.snapshots.load().revision();
    let epoch = context.runtime.scheduler_epoch();
    let incoming = token(
        "incoming-access",
        Some("shared-refresh"),
        None,
        Some("account-a"),
        Some("first@example.com"),
    );

    let result = context
        .publisher
        .activate_oauth_login(activation(OAuthAccountId::new(), incoming))
        .await;
    let stored = context
        .repository
        .load_configuration()
        .await
        .expect("stored configuration");

    assert!(matches!(
        result,
        Err(ConfigPublishError::OAuthAccountIdentityConflict)
    ));
    assert_eq!(first.revision().get(), 2);
    assert_eq!(revision.get(), 3);
    assert_eq!(context.snapshots.load().revision(), revision);
    assert_eq!(stored.revision(), revision);
    assert_eq!(stored.oauth_accounts().accounts().len(), 2);
    assert_eq!(context.runtime.scheduler_epoch(), epoch);
}

#[tokio::test]
async fn codex_team_members_are_independent_and_same_member_reauthorizes() {
    let context = TestContext::new().await;
    let first_id = OAuthAccountId::new();
    let first = codex_token("workspace-team", "member-a", "first@example.com");
    context
        .publisher
        .activate_oauth_account(
            first_id,
            ProviderKind::Codex,
            oauth_account_draft("Team member A"),
            first.email().map(str::to_owned),
            first.expires_at(),
            Vec::new(),
            document(&first),
        )
        .await
        .expect("first Team member");

    let second = codex_token("workspace-team", "member-b", "second@example.com");
    let (published, second_id) = context
        .publisher
        .activate_oauth_login(codex_activation(OAuthAccountId::new(), second))
        .await
        .expect("second Team member creates an account");
    assert_ne!(second_id, first_id);
    assert_eq!(published.oauth_accounts().accounts().len(), 2);
    assert_eq!(
        published
            .oauth_token_material(first_id)
            .expect("member A token")
            .account_id(),
        Some("workspace-team")
    );
    assert_eq!(
        published
            .oauth_token_material(second_id)
            .expect("member B token")
            .account_id(),
        Some("workspace-team")
    );

    let refreshed_member_a = codex_token("workspace-team", "member-a", "renamed@example.com");
    let (published, reauthorized_id) = context
        .publisher
        .activate_oauth_login(codex_activation(OAuthAccountId::new(), refreshed_member_a))
        .await
        .expect("same Team member reauthorizes");
    assert_eq!(reauthorized_id, first_id);
    assert_eq!(published.oauth_accounts().accounts().len(), 2);
}

fn activation(id: OAuthAccountId, token: OAuthTokenMaterial) -> OAuthAccountActivation {
    let document = document(&token);
    OAuthAccountActivation {
        id,
        provider_kind: token.provider(),
        preferred_label: token.email().map(str::to_owned),
        safe_account_email: token.email().map(str::to_owned),
        expires_at: token.expires_at(),
        models: Vec::new(),
        document,
        token,
    }
}

fn codex_activation(id: OAuthAccountId, token: OAuthTokenMaterial) -> OAuthAccountActivation {
    let document = document(&token);
    OAuthAccountActivation {
        id,
        provider_kind: token.provider(),
        preferred_label: token.email().map(str::to_owned),
        safe_account_email: token.email().map(str::to_owned),
        expires_at: token.expires_at(),
        models: Vec::new(),
        document,
        token,
    }
}

fn token(
    access: &str,
    refresh: Option<&str>,
    id_token: Option<&str>,
    account_id: Option<&str>,
    email: Option<&str>,
) -> OAuthTokenMaterial {
    OAuthTokenMaterial::new(
        ProviderKind::Claude,
        access.to_owned(),
        refresh.map(str::to_owned),
        id_token.map(str::to_owned),
        Some(1_900_000_000),
        account_id.map(str::to_owned),
        email.map(str::to_owned),
    )
    .expect("OAuth token")
}

fn codex_token(workspace: &str, member: &str, email: &str) -> OAuthTokenMaterial {
    let payload = URL_SAFE_NO_PAD.encode(
        serde_json::json!({
            "email": email,
            "https://api.openai.com/auth": {
                "chatgpt_account_id": workspace,
                "chatgpt_user_id": member,
                "chatgpt_plan_type": "team"
            }
        })
        .to_string(),
    );
    OAuthTokenMaterial::new(
        ProviderKind::Codex,
        format!("access-{member}"),
        Some(format!("refresh-{member}")),
        Some(format!("header.{payload}.signature")),
        Some(1_900_000_000),
        Some(workspace.to_owned()),
        Some(email.to_owned()),
    )
    .expect("Codex OAuth token")
}

fn document(token: &OAuthTokenMaterial) -> OAuthAccountDocument {
    OAuthAccountDocument::new(
        token.provider(),
        encode_oauth_account_document(token)
            .expect("OAuth document JSON")
            .into(),
    )
    .expect("OAuth account document")
}
