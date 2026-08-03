use any2api_domain::ConfigRevision;
use axum::{
    extract::{FromRequestParts, Query},
    http::request::Parts,
};
use serde::Deserialize;
use serde::de::DeserializeOwned;

use super::error::AdminApiError;

#[derive(Debug, Deserialize)]
pub(crate) struct ExpectedRevisionRequest {
    expected_revision: u64,
}

impl ExpectedRevisionRequest {
    pub(crate) fn revision(self) -> Result<ConfigRevision, AdminApiError> {
        parse_revision(self.expected_revision)
    }
}

#[derive(Debug, Deserialize)]
struct ExpectedRevisionQuery {
    expected_revision: u64,
}

impl ExpectedRevisionQuery {
    fn revision(self) -> Result<ConfigRevision, AdminApiError> {
        parse_revision(self.expected_revision)
    }
}

pub(crate) struct RequiredRevisionQuery(pub(crate) ConfigRevision);

impl<S> FromRequestParts<S> for RequiredRevisionQuery
where
    S: Send + Sync,
{
    type Rejection = AdminApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        let Query(query) = Query::<ExpectedRevisionQuery>::from_request_parts(parts, state)
            .await
            .map_err(|_| AdminApiError::invalid_request("expected_revision query is required"))?;
        Ok(Self(query.revision()?))
    }
}

pub(crate) struct RequiredVersionedQuery<T>(pub(crate) T);

impl<S, T> FromRequestParts<S> for RequiredVersionedQuery<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AdminApiError;

    async fn from_request_parts(parts: &mut Parts, state: &S) -> Result<Self, Self::Rejection> {
        Query::<T>::from_request_parts(parts, state)
            .await
            .map(|Query(query)| Self(query))
            .map_err(|_| {
                AdminApiError::invalid_request(
                    "expected_revision and expected_config_version queries are required",
                )
            })
    }
}

pub(crate) fn parse_revision(value: u64) -> Result<ConfigRevision, AdminApiError> {
    ConfigRevision::new(value)
        .map_err(|_| AdminApiError::invalid_request("expected_revision is invalid"))
}

#[cfg(test)]
mod tests {
    use axum::{
        extract::FromRequestParts,
        http::Request,
        response::{IntoResponse, Response},
    };
    use http_body_util::BodyExt;
    use serde::Deserialize;

    use super::{RequiredRevisionQuery, RequiredVersionedQuery};

    #[derive(Deserialize)]
    struct VersionedQuery {
        expected_revision: u64,
        expected_config_version: u64,
    }

    #[tokio::test]
    async fn required_revision_queries_have_one_stable_error_contract() {
        let mut parts = request_parts("/?expected_revision=7");
        let RequiredRevisionQuery(revision) =
            RequiredRevisionQuery::from_request_parts(&mut parts, &())
                .await
                .expect("revision query");
        assert_eq!(revision.get(), 7);

        let mut parts = request_parts("/");
        let error = match RequiredRevisionQuery::from_request_parts(&mut parts, &()).await {
            Ok(_) => panic!("missing revision query was accepted"),
            Err(error) => error,
        };
        assert_eq!(
            error_json(error.into_response()).await["error"]["message"],
            "expected_revision query is required"
        );

        let mut parts = request_parts("/?expected_revision=8&expected_config_version=3");
        let RequiredVersionedQuery(query) =
            RequiredVersionedQuery::<VersionedQuery>::from_request_parts(&mut parts, &())
                .await
                .expect("versioned query");
        assert_eq!(query.expected_revision, 8);
        assert_eq!(query.expected_config_version, 3);

        let mut parts = request_parts("/?expected_revision=8");
        let error =
            match RequiredVersionedQuery::<VersionedQuery>::from_request_parts(&mut parts, &())
                .await
            {
                Ok(_) => panic!("incomplete versioned query was accepted"),
                Err(error) => error,
            };
        assert_eq!(
            error_json(error.into_response()).await["error"]["message"],
            "expected_revision and expected_config_version queries are required"
        );
    }

    fn request_parts(uri: &str) -> axum::http::request::Parts {
        Request::builder()
            .uri(uri)
            .body(())
            .expect("request")
            .into_parts()
            .0
    }

    async fn error_json(response: Response) -> serde_json::Value {
        let body = response
            .into_body()
            .collect()
            .await
            .expect("error body")
            .to_bytes();
        serde_json::from_slice(&body).expect("error JSON")
    }
}
