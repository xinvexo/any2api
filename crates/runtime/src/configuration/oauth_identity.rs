use any2api_domain::ProviderKind;
use any2api_provider::api::OAuthTokenMaterial;

#[derive(Clone, Eq, PartialEq)]
pub(crate) struct OAuthAccountIdentity {
    provider: ProviderKind,
    key: OAuthAccountIdentityKey,
}

#[derive(Clone, Eq, PartialEq)]
enum OAuthAccountIdentityKey {
    AccountId(String),
    Email(String),
}

impl OAuthAccountIdentity {
    pub(crate) fn from_token(token: &OAuthTokenMaterial) -> Option<Self> {
        let key = token
            .account_id()
            .and_then(normalize_account_id)
            .map(OAuthAccountIdentityKey::AccountId)
            .or_else(|| {
                token
                    .email()
                    .and_then(normalize_email)
                    .map(OAuthAccountIdentityKey::Email)
            })?;
        Some(Self {
            provider: token.provider(),
            key,
        })
    }

    pub(crate) fn matches(&self, token: &OAuthTokenMaterial) -> bool {
        if token.provider() != self.provider {
            return false;
        }
        match &self.key {
            OAuthAccountIdentityKey::AccountId(expected) => token
                .account_id()
                .and_then(normalize_account_id)
                .is_some_and(|actual| actual == *expected),
            OAuthAccountIdentityKey::Email(expected) => {
                token.account_id().and_then(normalize_account_id).is_none()
                    && token
                        .email()
                        .and_then(normalize_email)
                        .is_some_and(|actual| actual == *expected)
            }
        }
    }
}

fn normalize_account_id(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_owned())
}

fn normalize_email(value: &str) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_lowercase())
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProviderKind;
    use any2api_provider::api::OAuthTokenMaterial;

    use super::OAuthAccountIdentity;

    fn token(
        provider: ProviderKind,
        account_id: Option<&str>,
        email: Option<&str>,
    ) -> OAuthTokenMaterial {
        OAuthTokenMaterial::new(
            provider,
            "access-token".into(),
            None,
            None,
            None,
            account_id.map(str::to_owned),
            email.map(str::to_owned),
        )
        .expect("token")
    }

    #[test]
    fn account_id_takes_priority_over_email_and_provider_isolated() {
        let identity = OAuthAccountIdentity::from_token(&token(
            ProviderKind::Codex,
            Some("account-a"),
            Some("person@example.com"),
        ))
        .expect("identity");

        assert!(identity.matches(&token(
            ProviderKind::Codex,
            Some("account-a"),
            Some("renamed@example.com"),
        )));
        assert!(!identity.matches(&token(
            ProviderKind::Codex,
            Some("account-b"),
            Some("person@example.com"),
        )));
        assert!(!identity.matches(&token(
            ProviderKind::Grok,
            Some("account-a"),
            Some("person@example.com"),
        )));
    }

    #[test]
    fn email_only_matches_another_token_without_account_id() {
        let identity = OAuthAccountIdentity::from_token(&token(
            ProviderKind::Claude,
            None,
            Some(" Person@Example.COM "),
        ))
        .expect("identity");

        assert!(identity.matches(&token(
            ProviderKind::Claude,
            None,
            Some("person@example.com"),
        )));
        assert!(!identity.matches(&token(
            ProviderKind::Claude,
            Some("stable-id"),
            Some("person@example.com"),
        )));
        assert!(
            OAuthAccountIdentity::from_token(&token(ProviderKind::Claude, None, None)).is_none()
        );
    }
}
