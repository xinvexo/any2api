use crate::admin_auth::AdminConnection;
use serde::{Deserialize, Serialize};

pub(super) struct PasswordRequest {
    pub(super) password: String,
}

pub(super) struct PasswordRotationRequest {
    pub(super) current_password: String,
    pub(super) new_password: String,
}

pub(super) struct SetupRequest {
    pub(super) setup_token: String,
    pub(super) password: String,
}

impl<'de> Deserialize<'de> for PasswordRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            password: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            password: wire.password,
        })
    }
}

impl<'de> Deserialize<'de> for PasswordRotationRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            current_password: String,
            new_password: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            current_password: wire.current_password,
            new_password: wire.new_password,
        })
    }
}

impl<'de> Deserialize<'de> for SetupRequest {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        #[derive(Deserialize)]
        struct Wire {
            setup_token: String,
            password: String,
        }

        let wire = Wire::deserialize(deserializer)?;
        Ok(Self {
            setup_token: wire.setup_token,
            password: wire.password,
        })
    }
}

#[derive(Serialize)]
pub(super) struct AdminSessionResponse {
    initialized: bool,
    authenticated: bool,
    csrf_token: Option<String>,
    remote_access_enabled: bool,
    secure_transport: bool,
    client_loopback: bool,
    through_trusted_proxy: bool,
    plaintext_http_warning: bool,
}

impl AdminSessionResponse {
    pub(super) fn new(
        initialized: bool,
        csrf_token: Option<String>,
        remote_access_enabled: bool,
        connection: AdminConnection,
    ) -> Self {
        let authenticated = csrf_token.is_some();
        Self {
            initialized,
            authenticated,
            csrf_token,
            remote_access_enabled,
            secure_transport: connection.is_secure(),
            client_loopback: connection.is_loopback(),
            through_trusted_proxy: connection.through_trusted_proxy(),
            plaintext_http_warning: !connection.is_loopback() && !connection.is_secure(),
        }
    }
}
