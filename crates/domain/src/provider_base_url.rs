use thiserror::Error;
use url::Url;

const MAX_PROVIDER_BASE_URL_CHARS: usize = 2_048;

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ProviderBaseUrl {
    value: String,
}

impl ProviderBaseUrl {
    pub fn parse(input: impl Into<String>) -> Result<Self, ProviderUrlValidationError> {
        let input = input.into();
        if input.is_empty() || input.trim() != input {
            return Err(ProviderUrlValidationError::NotTrimmed);
        }
        if input.chars().count() > MAX_PROVIDER_BASE_URL_CHARS {
            return Err(ProviderUrlValidationError::TooLong);
        }
        if raw_has_path_traversal(&input) {
            return Err(ProviderUrlValidationError::PathTraversalNotAllowed);
        }

        let mut url = Url::parse(&input).map_err(|_| ProviderUrlValidationError::Malformed)?;
        match url.scheme() {
            "http" | "https" => {}
            _ => return Err(ProviderUrlValidationError::UnsupportedScheme),
        }
        if url.host_str().is_none() {
            return Err(ProviderUrlValidationError::MissingHost);
        }
        if !url.username().is_empty() || url.password().is_some() {
            return Err(ProviderUrlValidationError::UserInfoNotAllowed);
        }
        if url.query().is_some() {
            return Err(ProviderUrlValidationError::QueryNotAllowed);
        }
        if url.fragment().is_some() {
            return Err(ProviderUrlValidationError::FragmentNotAllowed);
        }
        if url.port() == Some(0) {
            return Err(ProviderUrlValidationError::InvalidPort);
        }
        if has_path_traversal(&url) {
            return Err(ProviderUrlValidationError::PathTraversalNotAllowed);
        }
        let path = url.path().trim_end_matches('/').to_owned();
        url.set_path(if path.is_empty() { "/" } else { &path });
        let mut value = url.to_string();
        if url.path() == "/" {
            value.pop();
        }
        Ok(Self { value })
    }

    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.value
    }

    pub fn append_path(&self, suffix: &str) -> Result<String, ProviderUrlValidationError> {
        let suffix = suffix.trim_matches('/');
        if suffix.is_empty() || has_segment_traversal(suffix) {
            return Err(ProviderUrlValidationError::PathTraversalNotAllowed);
        }

        let mut url = Url::parse(&self.value).map_err(|_| ProviderUrlValidationError::Malformed)?;
        let prefix = url.path().trim_end_matches('/');
        let path = if prefix.is_empty() {
            format!("/{suffix}")
        } else {
            format!("{prefix}/{suffix}")
        };
        url.set_path(&path);
        Ok(url.to_string())
    }
}

#[derive(Clone, Debug, Error, Eq, PartialEq)]
pub enum ProviderUrlValidationError {
    #[error("provider base URL must not be empty or contain surrounding whitespace")]
    NotTrimmed,
    #[error("provider base URL is malformed")]
    Malformed,
    #[error("provider base URL is too long")]
    TooLong,
    #[error("provider base URL scheme must be http or https")]
    UnsupportedScheme,
    #[error("provider base URL must include a host")]
    MissingHost,
    #[error("provider base URL cannot contain userinfo")]
    UserInfoNotAllowed,
    #[error("provider base URL cannot contain a query")]
    QueryNotAllowed,
    #[error("provider base URL cannot contain a fragment")]
    FragmentNotAllowed,
    #[error("provider base URL port is invalid")]
    InvalidPort,
    #[error("provider base URL contains a traversal path segment")]
    PathTraversalNotAllowed,
}

fn has_path_traversal(url: &Url) -> bool {
    url.path_segments()
        .is_some_and(|mut segments| segments.any(|segment| segment == "." || segment == ".."))
        || url.path().to_ascii_lowercase().contains("%2e")
}

fn raw_has_path_traversal(input: &str) -> bool {
    let lower = input.to_ascii_lowercase();
    lower.contains("/../")
        || lower.contains("/./")
        || lower.ends_with("/..")
        || lower.ends_with("/.")
        || lower.contains("%2e")
}

fn has_segment_traversal(path: &str) -> bool {
    path.split('/')
        .any(|segment| segment == "." || segment == "..")
        || path.to_ascii_lowercase().contains("%2e")
}

#[cfg(test)]
mod tests {
    use super::{ProviderBaseUrl, ProviderUrlValidationError};

    fn parse(value: &str) -> Result<ProviderBaseUrl, ProviderUrlValidationError> {
        ProviderBaseUrl::parse(value)
    }

    #[test]
    fn preserves_path_prefix_and_normalizes_trailing_slash() {
        assert_eq!(
            parse("https://provider.example/")
                .expect("root URL")
                .as_str(),
            "https://provider.example"
        );
        let base = parse("https://provider.example/v1/").expect("base URL");

        assert_eq!(base.as_str(), "https://provider.example/v1");
        assert_eq!(
            base.append_path("responses").expect("joined URL"),
            "https://provider.example/v1/responses"
        );
    }

    #[test]
    fn rejects_unsafe_url_components() {
        assert_eq!(
            parse("https://user:pass@provider.example"),
            Err(ProviderUrlValidationError::UserInfoNotAllowed)
        );
        assert_eq!(
            parse("https://provider.example?token=secret"),
            Err(ProviderUrlValidationError::QueryNotAllowed)
        );
        assert_eq!(
            parse("https://provider.example/api/../admin"),
            Err(ProviderUrlValidationError::PathTraversalNotAllowed)
        );
    }

    #[test]
    fn accepts_http_and_private_targets_without_extra_authorization() {
        assert!(ProviderBaseUrl::parse("http://127.0.0.1:8080").is_ok());
        assert!(ProviderBaseUrl::parse("https://192.168.1.10/v1").is_ok());
        assert!(ProviderBaseUrl::parse("http://provider.internal/v1").is_ok());
    }
}
