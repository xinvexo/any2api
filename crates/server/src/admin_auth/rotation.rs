use std::{sync::Arc, time::Instant};

use super::{
    AdminAuthError, AdminAuthService, AdminSessionIssue,
    password::{hash_password, validate_new_password, verify_password},
    session::prepare as prepare_session,
};

impl AdminAuthService {
    pub async fn rotate_password(
        &self,
        current_password: String,
        new_password: String,
    ) -> Result<AdminSessionIssue, AdminAuthError> {
        validate_new_password(&new_password)?;
        // The credential lock serializes rotation with initialization; the
        // password-hash lock is only taken briefly around the in-memory swaps
        // so login and session checks are never blocked behind argon2 work or
        // storage IO. `store.replace` keeps the compare-and-swap semantics.
        let _credential_guard = self.credential_lock.lock().await;
        let current_hash = self
            .password_hash
            .read()
            .await
            .clone()
            .ok_or(AdminAuthError::NotInitialized)?;
        let password_check = Arc::clone(&self.password_checks)
            .try_acquire_owned()
            .map_err(|_| AdminAuthError::RateLimited { retry_after: 1 })?;
        if !verify_password(
            current_hash.clone(),
            current_password,
            password_check,
            &self.lifecycle,
        )
        .await?
        {
            return Err(AdminAuthError::CurrentPasswordInvalid);
        }

        let hash_check = Arc::clone(&self.setup_checks)
            .try_acquire_owned()
            .map_err(|_| AdminAuthError::RateLimited { retry_after: 1 })?;
        let new_hash = hash_password(new_password, hash_check, &self.lifecycle).await?;
        let (session_key, csrf, issue) = prepare_session()?;
        if !self
            .store
            .replace(&current_hash, &new_hash)
            .await
            .map_err(AdminAuthError::Store)?
        {
            let stored = self
                .store
                .load()
                .await
                .map_err(AdminAuthError::Store)?
                .ok_or(AdminAuthError::PasswordHash)?;
            *self.password_hash.write().await = Some(stored.as_str().to_owned());
            self.failures.lock().await.clear();
            self.sessions.lock().await.clear();
            return Err(AdminAuthError::CredentialChanged);
        }

        *self.password_hash.write().await = Some(new_hash);
        self.failures.lock().await.clear();
        self.sessions
            .lock()
            .await
            .replace_with(session_key, csrf, Instant::now());
        Ok(issue)
    }
}
