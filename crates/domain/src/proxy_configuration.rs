use std::collections::{HashMap, HashSet};

use crate::{ProxyProfile, ProxyProfileId, ProxyValidationError};

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyConfiguration {
    profiles: Vec<ProxyProfile>,
    global_proxy_id: ProxyProfileId,
}

impl ProxyConfiguration {
    pub fn new(
        mut profiles: Vec<ProxyProfile>,
        global_proxy_id: ProxyProfileId,
    ) -> Result<Self, ProxyValidationError> {
        validate_profiles(&profiles, global_proxy_id)?;
        profiles.sort_by(|left, right| {
            right
                .is_built_in()
                .cmp(&left.is_built_in())
                .then_with(|| left.name().cmp(right.name()))
        });

        Ok(Self {
            profiles,
            global_proxy_id,
        })
    }

    #[must_use]
    pub fn initial() -> Self {
        Self {
            profiles: vec![ProxyProfile::direct()],
            global_proxy_id: ProxyProfileId::DIRECT,
        }
    }

    #[must_use]
    pub fn profiles(&self) -> &[ProxyProfile] {
        &self.profiles
    }

    #[must_use]
    pub const fn global_proxy_id(&self) -> ProxyProfileId {
        self.global_proxy_id
    }

    #[must_use]
    pub fn get(&self, id: ProxyProfileId) -> Option<&ProxyProfile> {
        self.profiles.iter().find(|profile| profile.id() == id)
    }

    #[must_use]
    pub fn resolve(&self, credential_proxy_id: ProxyProfileId) -> Option<&ProxyProfile> {
        let resolved = if credential_proxy_id == ProxyProfileId::DIRECT {
            self.global_proxy_id
        } else {
            credential_proxy_id
        };

        self.get(resolved)
    }
}

fn validate_profiles(
    profiles: &[ProxyProfile],
    global_proxy_id: ProxyProfileId,
) -> Result<(), ProxyValidationError> {
    let mut ids = HashSet::new();
    let mut names = HashMap::new();

    for profile in profiles {
        if !ids.insert(profile.id()) {
            return Err(ProxyValidationError::DuplicateId);
        }
        if names.insert(profile.name_key(), profile.id()).is_some() {
            return Err(ProxyValidationError::DuplicateName);
        }
    }

    let direct = profiles
        .iter()
        .find(|profile| profile.id() == ProxyProfileId::DIRECT)
        .ok_or(ProxyValidationError::MissingDirect)?;
    if !direct.is_built_in() {
        return Err(ProxyValidationError::DirectInvariant);
    }

    let global = profiles
        .iter()
        .find(|profile| profile.id() == global_proxy_id)
        .ok_or(ProxyValidationError::GlobalProxyMissing)?;
    if !global.enabled() {
        return Err(ProxyValidationError::GlobalProxyDisabled);
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::{
        ProxyAddress, ProxyConfiguration, ProxyDraft, ProxyKind, ProxyProfile, ProxyProfileId,
        ProxyValidationError,
    };

    fn custom(name: &str) -> ProxyProfile {
        let address = ProxyAddress::new("127.0.0.1", 8080).expect("address");
        let draft = ProxyDraft::new(name, ProxyKind::Http, address, true).expect("draft");
        ProxyProfile::create(ProxyProfileId::new(), draft).expect("profile")
    }

    #[test]
    fn direct_binding_inherits_the_global_proxy() {
        let proxy = custom("Hong Kong");
        let proxy_id = proxy.id();
        let config = ProxyConfiguration::new(vec![ProxyProfile::direct(), proxy], proxy_id)
            .expect("configuration");

        assert_eq!(
            config.resolve(ProxyProfileId::DIRECT).map(ProxyProfile::id),
            Some(proxy_id)
        );
    }

    #[test]
    fn disabled_proxy_cannot_be_global() {
        let address = ProxyAddress::new("proxy.example.com", 1080).expect("address");
        let draft = ProxyDraft::new("Disabled", ProxyKind::Socks5, address, false).expect("draft");
        let proxy = ProxyProfile::create(ProxyProfileId::new(), draft).expect("profile");
        let proxy_id = proxy.id();

        let error = ProxyConfiguration::new(vec![ProxyProfile::direct(), proxy], proxy_id)
            .expect_err("disabled global must fail");

        assert_eq!(error, ProxyValidationError::GlobalProxyDisabled);
    }
}
