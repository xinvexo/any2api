use any2api_domain::OAuthAccount;
use any2api_provider::api::{OAuthTokenMaterial, ProviderDriver};
use base64::{Engine as _, engine::general_purpose::URL_SAFE_NO_PAD};
use sha2::{Digest, Sha256};

pub(super) fn fingerprint(
    account: &OAuthAccount,
    token: &OAuthTokenMaterial,
    driver: &dyn ProviderDriver,
) -> String {
    let mut hasher = Sha256::new();
    update(&mut hasher, account.provider_kind().as_str());
    if let Some(identity) = driver.oauth_principal_identity(token) {
        update(&mut hasher, "principal");
        hasher.update(identity.digest());
    } else {
        update(&mut hasher, "generation_and_token_version");
        update(&mut hasher, &account.account_generation().to_string());
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
    use any2api_domain::{
        OAuthAccountDraft, OAuthAccountId, OAuthProxySelection, ProviderKind, RequestsPerMinute,
    };

    #[test]
    fn stable_identity_ignores_token_and_routing_generation_changes() {
        let original = account(1, 1);
        let capabilities = crate::test_support::configuration_capabilities();
        let driver = capabilities
            .provider_registry()
            .get(ProviderKind::Codex)
            .expect("Codex driver");
        let original_token = token("access-a", Some("member-1"));
        let changed = account(2, 2);
        let changed_token = token("access-b", Some("member-1"));
        assert_eq!(
            fingerprint(&original, &original_token, driver.as_ref()),
            fingerprint(&changed, &changed_token, driver.as_ref())
        );
    }

    #[test]
    fn missing_stable_identity_uses_routing_and_token_generations() {
        let capabilities = crate::test_support::configuration_capabilities();
        let driver = capabilities
            .provider_registry()
            .get(ProviderKind::Codex)
            .expect("Codex driver");
        let original = fingerprint(&account(1, 1), &token("access-a", None), driver.as_ref());
        assert_ne!(
            original,
            fingerprint(&account(2, 1), &token("access-b", None), driver.as_ref())
        );
        assert_ne!(
            original,
            fingerprint(&account(1, 2), &token("access-a", None), driver.as_ref())
        );
    }

    #[test]
    fn same_workspace_different_members_have_different_fingerprints() {
        let capabilities = crate::test_support::configuration_capabilities();
        let driver = capabilities
            .provider_registry()
            .get(ProviderKind::Codex)
            .expect("Codex driver");
        let account = account(1, 1);
        assert_ne!(
            fingerprint(
                &account,
                &token("access-a", Some("member-a")),
                driver.as_ref(),
            ),
            fingerprint(
                &account,
                &token("access-b", Some("member-b")),
                driver.as_ref(),
            )
        );
    }

    fn account(token_version: u64, generation: u64) -> OAuthAccount {
        OAuthAccount::restore(
            OAuthAccountId::new(),
            ProviderKind::Codex,
            OAuthAccountDraft::new("test", Some(RequestsPerMinute::new(60).unwrap()), true)
                .unwrap(),
            OAuthProxySelection::Global,
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

    fn token(access_token: &str, member_id: Option<&str>) -> OAuthTokenMaterial {
        let id_token = member_id.map(|member_id| {
            let payload = base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(
                serde_json::json!({
                    "https://api.openai.com/auth": {
                        "chatgpt_account_id": "workspace-1",
                        "chatgpt_user_id": member_id,
                    }
                })
                .to_string(),
            );
            format!("header.{payload}.signature")
        });
        OAuthTokenMaterial::new(
            ProviderKind::Codex,
            access_token.into(),
            None,
            id_token,
            None,
            Some("workspace-1".into()),
            None,
        )
        .unwrap()
    }
}
