use std::fmt;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use hmac::{Hmac, Mac};
use secrecy::zeroize::Zeroizing;
use secrecy::{ExposeSecret, SecretSlice};
use sha2::Sha256;

use any2api_domain::{CredentialKind, ProviderKind};

use super::{
    context::{SecretContext, credential_kind_code, provider_kind_code},
    envelope::{NONCE_LENGTH, SecretEnvelope},
    error::SecretVaultError,
    master_key::MasterKey,
};

const VERIFIER_AAD: &[u8] = b"any2api-secret-vault-verifier-aad-v1";
const VERIFIER_PLAINTEXT: &[u8] = b"any2api-secret-vault-verifier-v1";
const FINGERPRINT_KEY_DOMAIN: &[u8] = b"any2api-provider-credential-fingerprint-key-v1";
const FINGERPRINT_DOMAIN: &[u8] = b"any2api-provider-credential-fingerprint-v1";

type HmacSha256 = Hmac<Sha256>;

pub type SecretBytes = SecretSlice<u8>;

pub struct SecretVault {
    master_key: MasterKey,
}

impl SecretVault {
    pub(super) const fn new(master_key: MasterKey) -> Self {
        Self { master_key }
    }

    #[must_use]
    pub fn key_id(&self) -> &str {
        self.master_key.key_id()
    }

    pub fn seal(
        &self,
        context: SecretContext,
        plaintext: &SecretBytes,
    ) -> Result<SecretEnvelope, SecretVaultError> {
        self.seal_with_aad(context.encode_aad(), plaintext.expose_secret())
    }

    pub fn open(
        &self,
        context: SecretContext,
        envelope: &SecretEnvelope,
    ) -> Result<SecretBytes, SecretVaultError> {
        self.open_with_aad(context.encode_aad(), envelope)
            .map(Into::into)
    }

    pub fn credential_fingerprint(
        &self,
        provider_kind: ProviderKind,
        credential_kind: CredentialKind,
        plaintext: &SecretBytes,
    ) -> [u8; 32] {
        let mut key_derivation = <HmacSha256 as Mac>::new_from_slice(self.master_key.expose())
            .expect("HMAC accepts any key length");
        key_derivation.update(FINGERPRINT_KEY_DOMAIN);
        let derived = key_derivation.finalize().into_bytes();
        let mut fingerprint_key = Zeroizing::new([0_u8; 32]);
        fingerprint_key.copy_from_slice(&derived);

        let mut mac = <HmacSha256 as Mac>::new_from_slice(fingerprint_key.as_ref())
            .expect("HMAC accepts any key length");
        mac.update(FINGERPRINT_DOMAIN);
        mac.update(&[provider_kind_code(provider_kind)]);
        mac.update(&[credential_kind_code(credential_kind)]);
        mac.update(plaintext.expose_secret());
        mac.finalize().into_bytes().into()
    }

    pub(crate) fn seal_verifier(&self) -> Result<SecretEnvelope, SecretVaultError> {
        self.seal_with_aad(VERIFIER_AAD, VERIFIER_PLAINTEXT)
    }

    pub(crate) fn verify(&self, envelope: &SecretEnvelope) -> Result<(), SecretVaultError> {
        let plaintext = self.open_with_aad(VERIFIER_AAD, envelope)?;
        if plaintext == VERIFIER_PLAINTEXT {
            Ok(())
        } else {
            Err(SecretVaultError::KeyMismatch)
        }
    }

    fn seal_with_aad(
        &self,
        aad: impl AsRef<[u8]>,
        plaintext: &[u8],
    ) -> Result<SecretEnvelope, SecretVaultError> {
        let mut nonce = [0_u8; NONCE_LENGTH];
        getrandom::fill(&mut nonce).map_err(|_| SecretVaultError::RandomGeneration)?;
        let cipher = XChaCha20Poly1305::new_from_slice(self.master_key.expose())
            .map_err(|_| SecretVaultError::EncryptionFailed)?;
        let ciphertext = cipher
            .encrypt(
                XNonce::from_slice(&nonce),
                Payload {
                    msg: plaintext,
                    aad: aad.as_ref(),
                },
            )
            .map_err(|_| SecretVaultError::EncryptionFailed)?;
        Ok(SecretEnvelope::new(
            self.master_key.key_id().to_owned(),
            nonce,
            ciphertext,
        ))
    }

    fn open_with_aad(
        &self,
        aad: impl AsRef<[u8]>,
        envelope: &SecretEnvelope,
    ) -> Result<Vec<u8>, SecretVaultError> {
        if envelope.key_id() != self.master_key.key_id() {
            return Err(SecretVaultError::KeyMismatch);
        }
        let cipher = XChaCha20Poly1305::new_from_slice(self.master_key.expose())
            .map_err(|_| SecretVaultError::AuthenticationFailed)?;
        cipher
            .decrypt(
                XNonce::from_slice(envelope.nonce()),
                Payload {
                    msg: envelope.ciphertext(),
                    aad: aad.as_ref(),
                },
            )
            .map_err(|_| SecretVaultError::AuthenticationFailed)
    }
}

impl fmt::Debug for SecretVault {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("SecretVault")
            .field("key_id", &self.master_key.key_id())
            .field("master_key", &"[REDACTED]")
            .finish()
    }
}
