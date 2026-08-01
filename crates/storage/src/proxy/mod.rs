mod authentication;
mod authentication_writes;
mod mutation;
mod password;
mod password_material;
mod repository;
mod rows;
mod writes;

#[cfg(test)]
mod tests;

pub use password::ProxyPasswordValidationError;
pub use password_material::{StoredProxyPassword, StoredProxyPasswords};

pub(crate) use authentication::{
    ProxyAuthenticationMutation, mutate_connection as mutate_proxy_authentication_configuration,
};
pub(crate) use mutation::ProxyMutation;
pub(crate) use repository::mutate_connection as mutate_proxy_configuration;
pub(crate) use rows::load_proxies_from;
