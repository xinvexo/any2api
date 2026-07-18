use any2api_domain::{CredentialId, CredentialKind, ProviderKind};
use secrecy::ExposeSecret;
use tempfile::tempdir;

use super::{
    SecretBytes, SecretContext, SecretEnvelope, SecretVault, context::AAD_VERSION,
    envelope::ENVELOPE_VERSION, error::SecretVaultError, master_key::MasterKey,
};

#[test]
fn secret_round_trip_uses_unique_nonces() {
    let directory = tempdir().expect("temporary directory");
    let master_key = MasterKey::load_or_create(&directory.path().join("master-key.json"), true)
        .expect("master key");
    let vault = SecretVault::new(master_key);
    let context = provider_context(CredentialId::new());
    let secret: SecretBytes = b"provider-api-key".to_vec().into();

    let first = vault.seal(context, &secret).expect("first envelope");
    let second = vault.seal(context, &secret).expect("second envelope");
    let opened = vault.open(context, &first).expect("opened secret");

    assert_ne!(first.nonce(), second.nonce());
    assert_ne!(first.ciphertext(), second.ciphertext());
    assert_eq!(opened.expose_secret(), b"provider-api-key");
}

#[test]
fn wrong_aad_and_ciphertext_tampering_are_rejected() {
    let directory = tempdir().expect("temporary directory");
    let master_key = MasterKey::load_or_create(&directory.path().join("master-key.json"), true)
        .expect("master key");
    let vault = SecretVault::new(master_key);
    let context = provider_context(CredentialId::new());
    let secret: SecretBytes = b"provider-api-key".to_vec().into();
    let envelope = vault.seal(context, &secret).expect("envelope");

    let wrong_context = provider_context(CredentialId::new());
    assert!(matches!(
        vault.open(wrong_context, &envelope),
        Err(SecretVaultError::AuthenticationFailed)
    ));

    let mut ciphertext = envelope.ciphertext().to_vec();
    ciphertext[0] ^= 1;
    let tampered = SecretEnvelope::restore(
        envelope.version(),
        envelope.key_id(),
        envelope.algorithm().as_str(),
        envelope.nonce(),
        ciphertext,
        envelope.aad_version(),
    )
    .expect("structurally valid envelope");
    assert!(matches!(
        vault.open(context, &tampered),
        Err(SecretVaultError::AuthenticationFailed)
    ));
}

#[test]
fn envelope_versions_are_strict_and_debug_is_redacted() {
    let error = SecretEnvelope::restore(
        ENVELOPE_VERSION + 1,
        "mk1_example",
        "xchacha20poly1305",
        &[0; 24],
        vec![0; 16],
        AAD_VERSION,
    )
    .expect_err("unknown envelope version must fail");
    assert!(matches!(
        error,
        SecretVaultError::UnsupportedEnvelopeVersion
    ));
    assert!(matches!(
        SecretEnvelope::restore(
            ENVELOPE_VERSION,
            "mk1_example",
            "unknown",
            &[0; 24],
            vec![0; 16],
            AAD_VERSION,
        ),
        Err(SecretVaultError::UnsupportedEnvelopeAlgorithm)
    ));
    assert!(matches!(
        SecretEnvelope::restore(
            ENVELOPE_VERSION,
            "mk1_example",
            "xchacha20poly1305",
            &[0; 24],
            vec![0; 16],
            AAD_VERSION + 1,
        ),
        Err(SecretVaultError::UnsupportedAadVersion)
    ));

    let envelope = SecretEnvelope::restore(
        ENVELOPE_VERSION,
        "mk1_example",
        "xchacha20poly1305",
        &[7; 24],
        vec![9; 32],
        AAD_VERSION,
    )
    .expect("envelope");
    let debug = format!("{envelope:?}");
    assert!(!debug.contains("[7, 7"));
    assert!(!debug.contains("[9, 9"));
    assert!(debug.contains("REDACTED"));
}

fn provider_context(credential_id: CredentialId) -> SecretContext {
    SecretContext::provider_credential(credential_id, ProviderKind::Codex, CredentialKind::ApiKey)
}
