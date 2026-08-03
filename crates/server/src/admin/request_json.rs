use axum::{
    Json,
    extract::{FromRequest, Request},
};
use serde::de::DeserializeOwned;

use super::error::AdminApiError;

/// Management JSON body with the namespace's stable error envelope.
pub(crate) struct AdminJson<T>(pub(crate) T);

impl<S, T> FromRequest<S> for AdminJson<T>
where
    S: Send + Sync,
    T: DeserializeOwned,
{
    type Rejection = AdminApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        Json::<T>::from_request(request, state)
            .await
            .map(|Json(value)| Self(value))
            .map_err(|_| AdminApiError::invalid_request("request body must be valid JSON"))
    }
}

#[cfg(test)]
mod tests {
    use axum::{
        body::Body,
        extract::FromRequest,
        http::{Request, StatusCode, header::CONTENT_TYPE},
        response::IntoResponse,
    };
    use http_body_util::BodyExt;
    use serde::Deserialize;

    use super::AdminJson;

    #[derive(Debug, Deserialize)]
    struct Payload {
        value: u64,
    }

    #[tokio::test]
    async fn maps_json_rejections_to_the_stable_admin_error() {
        let request = Request::builder()
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from("{"))
            .expect("request");
        let error = match AdminJson::<Payload>::from_request(request, &()).await {
            Ok(_) => panic!("invalid JSON was accepted"),
            Err(error) => error,
        };
        let response = error.into_response();
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let body = response
            .into_body()
            .collect()
            .await
            .expect("error body")
            .to_bytes();
        let body: serde_json::Value = serde_json::from_slice(&body).expect("error JSON");
        assert_eq!(body["error"]["code"], "invalid_request");
        assert_eq!(body["error"]["message"], "request body must be valid JSON");

        let request = Request::builder()
            .header(CONTENT_TYPE, "application/json")
            .body(Body::from(r#"{"value":7}"#))
            .expect("request");
        let AdminJson(payload) = AdminJson::<Payload>::from_request(request, &())
            .await
            .expect("valid JSON");
        assert_eq!(payload.value, 7);
    }
}
