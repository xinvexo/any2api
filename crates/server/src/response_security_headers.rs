use axum::{
    extract::Request,
    http::{HeaderMap, HeaderValue, header::HeaderName},
    middleware::Next,
    response::Response,
};

const X_CONTENT_TYPE_OPTIONS: HeaderName = HeaderName::from_static("x-content-type-options");

pub(crate) async fn add_nosniff(request: Request, next: Next) -> Response {
    let mut response = next.run(request).await;
    insert_nosniff(response.headers_mut());
    response
}

fn insert_nosniff(headers: &mut HeaderMap) {
    headers.insert(X_CONTENT_TYPE_OPTIONS, HeaderValue::from_static("nosniff"));
}

#[cfg(test)]
mod tests {
    use axum::http::{HeaderMap, HeaderValue};

    use super::insert_nosniff;

    #[test]
    fn nosniff_is_the_single_canonical_value() {
        let mut headers = HeaderMap::new();
        headers.append("x-content-type-options", HeaderValue::from_static("legacy"));
        headers.append(
            "x-content-type-options",
            HeaderValue::from_static("duplicate"),
        );

        insert_nosniff(&mut headers);

        assert_eq!(headers["x-content-type-options"], "nosniff");
        assert_eq!(headers.get_all("x-content-type-options").iter().count(), 1);
    }
}
