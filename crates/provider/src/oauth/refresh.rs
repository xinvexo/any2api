use http::StatusCode;
use serde::Deserialize;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum OAuthRefreshRejection {
    InvalidGrant,
    RefreshTokenExpired,
    RefreshTokenReused,
    RefreshTokenInvalidated,
    Unverified,
}

impl OAuthRefreshRejection {
    pub fn classify(status: StatusCode, bounded_body: &[u8]) -> Self {
        Self::classify_with_codes(
            status,
            bounded_body,
            &[("invalid_grant", Self::InvalidGrant)],
        )
    }

    pub(crate) fn classify_with_codes(
        status: StatusCode,
        bounded_body: &[u8],
        permanent_codes: &[(&str, Self)],
    ) -> Self {
        if status != StatusCode::BAD_REQUEST && status != StatusCode::UNAUTHORIZED {
            return Self::Unverified;
        }
        let Ok(envelope) = serde_json::from_slice::<RefreshErrorEnvelope>(bounded_body) else {
            return Self::Unverified;
        };
        permanent_codes
            .iter()
            .find_map(|(code, rejection)| envelope.has_code(code).then_some(*rejection))
            .unwrap_or(Self::Unverified)
    }

    #[must_use]
    pub const fn is_permanent(self) -> bool {
        !matches!(self, Self::Unverified)
    }
}

#[derive(Deserialize)]
struct RefreshErrorEnvelope {
    #[serde(default)]
    code: Option<String>,
    #[serde(default)]
    error: Option<RefreshError>,
}

impl RefreshErrorEnvelope {
    fn has_code(&self, expected: &str) -> bool {
        self.code.as_deref() == Some(expected)
            || self
                .error
                .as_ref()
                .is_some_and(|error| error.has_code(expected))
    }
}

#[derive(Deserialize)]
#[serde(untagged)]
enum RefreshError {
    Code(String),
    Details {
        #[serde(default)]
        code: Option<String>,
        #[serde(default, rename = "type")]
        kind: Option<String>,
    },
}

impl RefreshError {
    fn has_code(&self, expected: &str) -> bool {
        match self {
            Self::Code(code) => code == expected,
            Self::Details { code, kind } => {
                code.as_deref() == Some(expected) || kind.as_deref() == Some(expected)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::OAuthRefreshRejection;
    use http::StatusCode;

    #[test]
    fn recognizes_standard_and_structured_invalid_grant_errors() {
        for body in [
            br#"{"error":"invalid_grant"}"#.as_slice(),
            br#"{"error":{"type":"invalid_grant"}}"#.as_slice(),
            br#"{"code":"invalid_grant","error":{"message":"expired"}}"#.as_slice(),
            br#"{"code":"invalid_grant"}"#.as_slice(),
        ] {
            assert_eq!(
                OAuthRefreshRejection::classify(StatusCode::BAD_REQUEST, body),
                OAuthRefreshRejection::InvalidGrant
            );
        }
    }

    #[test]
    fn does_not_infer_invalid_grant_from_untrusted_messages_or_transient_statuses() {
        for (status, body) in [
            (
                StatusCode::BAD_REQUEST,
                br#"{"error":"invalid_client","error_description":"invalid_grant"}"#.as_slice(),
            ),
            (
                StatusCode::BAD_REQUEST,
                br#"{"error":{"message":"invalid_grant"}}"#.as_slice(),
            ),
            (
                StatusCode::SERVICE_UNAVAILABLE,
                br#"{"error":"invalid_grant"}"#.as_slice(),
            ),
            (StatusCode::BAD_REQUEST, br#"{"code":"unknown"}"#.as_slice()),
            (StatusCode::BAD_REQUEST, br#"{"code":42}"#.as_slice()),
            (StatusCode::BAD_REQUEST, br#"{}"#.as_slice()),
        ] {
            assert_eq!(
                OAuthRefreshRejection::classify(status, body),
                OAuthRefreshRejection::Unverified
            );
        }
    }
}
