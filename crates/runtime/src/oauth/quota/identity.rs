use any2api_domain::OAuthAccount;
use any2api_provider::api::OAuthTokenMaterial;
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

pub(super) fn fingerprint(account: &OAuthAccount, token: &OAuthTokenMaterial) -> String {
    let mut hasher = Sha256::new();
    update(&mut hasher, account.provider_kind().as_str());
    update(&mut hasher, &account.account_generation().to_string());
    if let Some(account_id) = token.account_id() {
        update(&mut hasher, "account_id");
        update(&mut hasher, account_id);
    } else if let Some(email) = token.email() {
        update(&mut hasher, "email");
        update(&mut hasher, &email.to_lowercase());
    } else {
        update(&mut hasher, "token_version");
        update(&mut hasher, &account.token_version().to_string());
    }
    format!("sha256:{}", URL_SAFE_NO_PAD.encode(hasher.finalize()))
}

fn update(hasher: &mut Sha256, value: &str) {
    hasher.update(value.len().to_le_bytes());
    hasher.update(value.as_bytes());
}

#[cfg(test)]
mod tests {
    use super::*;
    use any2api_domain::{OAuthAccountDraft, OAuthAccountId, ProviderKind, RequestsPerMinute};

    #[test]
    fn stable_identity_ignores_token_rotation_but_generation_isolated() {
        let original = account(1, 1);
        let token = token(Some("account-1"));
        let rotated = account(2, 1);
        assert_eq!(
            fingerprint(&original, &token),
            fingerprint(&rotated, &token)
        );
        let next_generation = account(2, 2);
        assert_ne!(
            fingerprint(&original, &token),
            fingerprint(&next_generation, &token)
        );
    }

    #[test]
    fn missing_stable_identity_uses_token_version() {
        assert_ne!(
            fingerprint(&account(1, 1), &token(None)),
            fingerprint(&account(2, 1), &token(None))
        );
    }

    fn account(token_version: u64, generation: u64) -> OAuthAccount {
        OAuthAccount::restore(
            OAuthAccountId::new(),
            ProviderKind::Codex,
            OAuthAccountDraft::new("test", Some(RequestsPerMinute::new(60).unwrap()), true)
                .unwrap(),
            any2api_domain::ProxyProfileId::DIRECT,
            None,
            None,
            "2026-08-11 00:00:00".into(),
            token_version,
            generation,
            1,
            vec!["gpt-5.6".into()],
        )
        .unwrap()
    }

    fn token(account_id: Option<&str>) -> OAuthTokenMaterial {
        OAuthTokenMaterial::new(
            ProviderKind::Codex,
            "access".into(),
            None,
            None,
            None,
            account_id.map(str::to_owned),
            None,
        )
        .unwrap()
    }
}
