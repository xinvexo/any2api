use std::fmt;

use secrecy::{ExposeSecret, SecretString};

pub struct ProxyCredentials {
    username: String,
    password: SecretString,
}

impl ProxyCredentials {
    #[must_use]
    pub fn new(username: String, password: String) -> Self {
        Self {
            username,
            password: password.into(),
        }
    }

    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }

    pub(crate) fn password(&self) -> &str {
        self.password.expose_secret()
    }
}

impl fmt::Debug for ProxyCredentials {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("ProxyCredentials")
            .field("username", &self.username)
            .field("password", &"[REDACTED]")
            .finish()
    }
}
