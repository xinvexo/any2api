use std::fmt;

use chacha20poly1305::{
    XChaCha20Poly1305, XNonce,
    aead::{Aead, KeyInit, Payload},
};
use secrecy::{ExposeSecret, SecretSlice};

use super::{
    context::SecretContext,
    envelope::{NONCE_LENGTH, SecretEnvelope},
    error::SecretVaultError,
    master_key::MasterKey,
};

const VERIFIER_AAD: &[u8] = b"any2api-secret-vault-verifier-aad-v1";
const VERIFIER_PLAINTEXT: &[u8] = b"any2api-secret-vault-verifier-v1";

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
