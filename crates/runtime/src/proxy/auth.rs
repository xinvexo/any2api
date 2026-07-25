use std::fmt;

use any2api_domain::{ProxyConfiguration, ProxyProfile, ProxyProfileId};
use any2api_storage::api::{StoredProxyPassword, StoredProxyPasswords};
use any2api_transport::api::ProxyCredentials;
use secrecy::ExposeSecret;

struct ProxyAuthMaterial {
    proxy_profile_id: ProxyProfileId,
    authentication_version: u64,
    credentials: ProxyCredentials,
}

impl ProxyAuthMaterial {
    fn from_stored(configuration: &ProxyConfiguration, stored: StoredProxyPassword) -> Self {
        let profile = configuration
            .get(stored.proxy_profile_id())
            .expect("storage validated proxy password owner");
        let authentication = profile
            .authentication()
            .expect("storage validated proxy authentication metadata");
        assert_eq!(
            stored.authentication_version(),
            profile.authentication_version(),
            "storage returned mismatched proxy authentication version"
        );
        let secret = stored.into_secret();
        let password = String::from_utf8(secret.expose_secret().to_vec())
            .expect("storage validated proxy password as UTF-8");
        Self {
            proxy_profile_id: profile.id(),
            authentication_version: profile.authentication_version(),
            credentials: ProxyCredentials::new(authentication.username().to_owned(), password),
        }
    }
}

pub(crate) struct ProxyAuthMaterials {
    entries: Vec<ProxyAuthMaterial>,
}

impl ProxyAuthMaterials {
    pub(crate) fn from_stored(
        configuration: &ProxyConfiguration,
        stored: StoredProxyPasswords,
    ) -> Self {
        Self {
            entries: stored
                .into_entries()
                .into_iter()
                .map(|entry| ProxyAuthMaterial::from_stored(configuration, entry))
                .collect(),
        }
    }

    pub(crate) fn credentials_for(&self, profile: &ProxyProfile) -> Option<&ProxyCredentials> {
        let authentication = profile.authentication()?;
        let entry = self
            .entries
            .iter()
            .find(|entry| entry.proxy_profile_id == profile.id())
            .expect("published authenticated proxy has password material");
        assert_eq!(
            entry.authentication_version,
            profile.authentication_version(),
            "published proxy password version matches profile"
        );
        assert_eq!(
            entry.credentials.username(),
            authentication.username(),
            "published proxy username matches profile"
        );
        Some(&entry.credentials)
    }
}

impl fmt::Debug for ProxyAuthMaterials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProxyAuthMaterials")
            .field("entry_count", &self.entries.len())
            .finish()
    }
}
