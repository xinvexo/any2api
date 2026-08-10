use std::sync::LazyLock;

use http::{HeaderMap, HeaderName};

use crate::header_policy::{ordered_names, project};

static RESPONSE_HEADERS: LazyLock<Vec<HeaderName>> = LazyLock::new(|| {
    ordered_names(&[
        "content-encoding",
        "content-type",
        "x-request-id",
        "request-id",
        "retry-after",
    ])
});

pub(crate) fn response(upstream: &HeaderMap) -> HeaderMap {
    project(upstream, RESPONSE_HEADERS.iter(), &["x-ratelimit-"])
}
