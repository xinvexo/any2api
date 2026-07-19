use argon2::{Argon2, PasswordHash, PasswordHasher, PasswordVerifier, password_hash::SaltString};
use tokio::sync::OwnedSemaphorePermit;

use super::AdminAuthError;

const MIN_PASSWORD_BYTES: usize = 12;
const MAX_PASSWORD_BYTES: usize = 1024;

pub(super) fn validate_new_password(password: &str) -> Result<(), AdminAuthError> {
    if !(MIN_PASSWORD_BYTES..=MAX_PASSWORD_BYTES).contains(&password.len()) {
        return Err(AdminAuthError::InvalidPassword);
    }
    Ok(())
}

pub(super) async fn hash_password(
    password: String,
    password_check: OwnedSemaphorePermit,
) -> Result<String, AdminAuthError> {
    tokio::task::spawn_blocking(move || {
        let _password_check = password_check;
        let mut salt = [0_u8; 16];
        getrandom::fill(&mut salt).map_err(|_| AdminAuthError::Random)?;
        let salt = SaltString::encode_b64(&salt).map_err(|_| AdminAuthError::PasswordHash)?;
        Argon2::default()
            .hash_password(password.as_bytes(), &salt)
            .map(|hash| hash.to_string())
            .map_err(|_| AdminAuthError::PasswordHash)
    })
    .await
    .map_err(|_| AdminAuthError::PasswordTask)?
}

pub(super) async fn verify_password(
    password_hash: String,
    password: String,
    password_check: OwnedSemaphorePermit,
) -> Result<bool, AdminAuthError> {
    tokio::task::spawn_blocking(move || {
        let _password_check = password_check;
        let parsed = PasswordHash::new(&password_hash).map_err(|_| AdminAuthError::PasswordHash)?;
        Ok(Argon2::default()
            .verify_password(password.as_bytes(), &parsed)
            .is_ok())
    })
    .await
    .map_err(|_| AdminAuthError::PasswordTask)?
}
