use secrecy::ExposeSecret;
use thiserror::Error;

use crate::vault::SecretBytes;

const MAX_PROXY_PASSWORD_BYTES: usize = 255;

pub(crate) fn validate(secret: &SecretBytes) -> Result<(), ProxyPasswordValidationError> {
    let value = secret.expose_secret();
    if value.is_empty() {
        return Err(ProxyPasswordValidationError::Empty);
    }
    if value.len() > MAX_PROXY_PASSWORD_BYTES {
        return Err(ProxyPasswordValidationError::TooLong);
    }
    std::str::from_utf8(value).map_err(|_| ProxyPasswordValidationError::InvalidUtf8)?;
    Ok(())
}

#[derive(Clone, Copy, Debug, Error, Eq, PartialEq)]
pub enum ProxyPasswordValidationError {
    #[error("proxy password must not be empty")]
    Empty,
    #[error("proxy password is too long")]
    TooLong,
    #[error("proxy password must be valid UTF-8")]
    InvalidUtf8,
}

#[cfg(test)]
mod tests {
    use super::{ProxyPasswordValidationError, validate};

    #[test]
    fn validates_the_socks5_password_length_boundary() {
        assert_eq!(
            validate(&Vec::new().into()),
            Err(ProxyPasswordValidationError::Empty)
        );
        assert!(validate(&vec![b'x'; 255].into()).is_ok());
        assert_eq!(
            validate(&vec![b'x'; 256].into()),
            Err(ProxyPasswordValidationError::TooLong)
        );
    }
}
