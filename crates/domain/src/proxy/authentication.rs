use thiserror::Error;

pub const MAX_PROXY_USERNAME_BYTES: usize = 255;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProxyAuthentication {
    username: String,
}

impl ProxyAuthentication {
    pub fn new(username: impl Into<String>) -> Result<Self, ProxyAuthenticationValidationError> {
        let username = username.into();
        validate_username(&username)?;
        Ok(Self { username })
    }

    #[must_use]
    pub fn username(&self) -> &str {
        &self.username
    }
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ProxyAuthenticationValidationError {
    #[error("proxy username must not be empty")]
    EmptyUsername,
    #[error("proxy username is too long")]
    UsernameTooLong,
    #[error("proxy username contains a control character")]
    UsernameControlCharacter,
    #[error("proxy username contains the HTTP Basic separator")]
    UsernameContainsBasicSeparator,
}

fn validate_username(username: &str) -> Result<(), ProxyAuthenticationValidationError> {
    if username.is_empty() {
        return Err(ProxyAuthenticationValidationError::EmptyUsername);
    }
    if username.len() > MAX_PROXY_USERNAME_BYTES {
        return Err(ProxyAuthenticationValidationError::UsernameTooLong);
    }
    if username.chars().any(char::is_control) {
        return Err(ProxyAuthenticationValidationError::UsernameControlCharacter);
    }
    if username.contains(':') {
        return Err(ProxyAuthenticationValidationError::UsernameContainsBasicSeparator);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{ProxyAuthentication, ProxyAuthenticationValidationError};

    #[test]
    fn validates_username_at_the_protocol_boundary() {
        assert_eq!(
            ProxyAuthentication::new("").expect_err("empty username must fail"),
            ProxyAuthenticationValidationError::EmptyUsername
        );
        assert_eq!(
            ProxyAuthentication::new("user\n").expect_err("control character must fail"),
            ProxyAuthenticationValidationError::UsernameControlCharacter
        );
        assert_eq!(
            ProxyAuthentication::new("proxy:user").expect_err("Basic separator must fail"),
            ProxyAuthenticationValidationError::UsernameContainsBasicSeparator
        );
        assert!(ProxyAuthentication::new("proxy-user").is_ok());
    }
}
