use any2api_domain::{
    ConfigRevision, CredentialId, CredentialKind, MaxConcurrency, ProtocolDialect,
    ProviderCredentialDraft, ProviderEndpointDraft, ProviderEndpointId, ProviderKind,
    ProxyProfileId,
};
use tempfile::tempdir;

use crate::api::{ConfigurationRepository, SecretBytes, SqliteStore};

#[tokio::test]
async fn oauth_credential_round_trip_and_refresh_preserve_selected_models() {
    let directory = tempdir().expect("temporary directory");
    let database = directory.path().join("config.sqlite3");
    let store = SqliteStore::connect(&database).await.expect("store");
    let endpoint_id = ProviderEndpointId::new();
    let credential_id = CredentialId::new();
    let endpoint = store
        .create_provider_endpoint(ConfigRevision::INITIAL, endpoint_id, endpoint_draft())
        .await
        .expect("create endpoint");
    let created = store
        .create_provider_oauth_credential(
            endpoint.revision(),
            credential_id,
            endpoint_id,
            1,
            credential_draft(),
            secret(OLD_SECRET),
        )
        .await
        .expect("create OAuth credential");
    let credential = created
        .provider_credentials()
        .get(credential_id)
        .expect("OAuth credential");
    assert_eq!(credential.credential_kind(), CredentialKind::OAuth2);
    assert_eq!(credential.fingerprint().tail(), None);

    let modeled = store
        .set_provider_credential_models(
            created.revision(),
            credential_id,
            credential.config_version(),
            vec!["gpt-5.1-codex".to_owned()],
        )
        .await
        .expect("save models");
    let refreshed = store
        .refresh_provider_oauth_credential_secret(
            modeled.revision(),
            credential_id,
            1,
            secret(NEW_SECRET),
        )
        .await
        .expect("refresh OAuth credential");
    let credential = refreshed
        .provider_credentials()
        .get(credential_id)
        .expect("refreshed credential");
    assert_eq!(credential.secret_version(), 2);
    assert_eq!(credential.credential_generation(), 2);
    assert_eq!(credential.models()[0].as_str(), "gpt-5.1-codex");
    assert_eq!(credential.fingerprint().tail(), None);
    assert_eq!(
        refreshed
            .provider_credential_secrets()
            .get(credential_id)
            .expect("OAuth secret")
            .expose_for_test(),
        NEW_SECRET.as_bytes()
    );

    let reopened = SqliteStore::connect(&database)
        .await
        .expect("reopened store")
        .load_configuration()
        .await
        .expect("reopened configuration");
    let credential = reopened
        .provider_credentials()
        .get(credential_id)
        .expect("reopened OAuth credential");
    assert_eq!(credential.credential_kind(), CredentialKind::OAuth2);
    assert_eq!(credential.secret_version(), 2);
    assert_eq!(credential.models()[0].as_str(), "gpt-5.1-codex");
}

fn credential_draft() -> ProviderCredentialDraft {
    ProviderCredentialDraft::new(
        "Personal OAuth",
        CredentialKind::OAuth2,
        ProxyProfileId::DIRECT,
        MaxConcurrency::new(2).expect("max concurrency"),
        true,
    )
    .expect("OAuth credential draft")
}

fn endpoint_draft() -> ProviderEndpointDraft {
    ProviderEndpointDraft::new(
        "Codex OAuth",
        ProviderKind::Codex,
        "https://chatgpt.com/backend-api/codex",
        ProtocolDialect::OpenAiResponses,
        true,
    )
    .expect("endpoint draft")
}

fn secret(value: &str) -> SecretBytes {
    value.as_bytes().to_vec().into()
}

const OLD_SECRET: &str = r#"{"provider":"codex","access_token":"old-access","refresh_token":"old-refresh","id_token":null,"expires_at":1,"account_id":null,"email":null,"organization_id":null,"client_id":null}"#;
const NEW_SECRET: &str = r#"{"provider":"codex","access_token":"new-access","refresh_token":"new-refresh","id_token":null,"expires_at":9999999999,"account_id":null,"email":null,"organization_id":null,"client_id":null}"#;
