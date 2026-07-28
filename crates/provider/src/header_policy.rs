use http::{HeaderMap, HeaderName, HeaderValue, header};

const MAX_FORWARDED_HEADER_VALUES: usize = 64;
const MAX_FORWARDED_HEADER_VALUE_BYTES: usize = 8 * 1024;
const MAX_FORWARDED_HEADER_BYTES: usize = 32 * 1024;

pub(crate) fn project(source: &HeaderMap, exact: &[&str], prefixes: &[&str]) -> HeaderMap {
    let connection_nominated = connection_nominated(source);
    let mut projected = HeaderMap::new();
    let mut values = 0_usize;
    let mut bytes = 0_usize;
    for (name, value) in source {
        let name_text = name.as_str();
        if forbidden(name_text)
            || connection_nominated
                .iter()
                .any(|nominated| nominated == name)
            || (!exact
                .iter()
                .any(|allowed| name_text.eq_ignore_ascii_case(allowed))
                && !prefixes.iter().any(|prefix| name_text.starts_with(prefix)))
            || value.as_bytes().len() > MAX_FORWARDED_HEADER_VALUE_BYTES
        {
            continue;
        }
        let next_bytes = bytes
            .saturating_add(name_text.len())
            .saturating_add(value.as_bytes().len());
        if values >= MAX_FORWARDED_HEADER_VALUES || next_bytes > MAX_FORWARDED_HEADER_BYTES {
            break;
        }
        projected.append(name.clone(), value.clone());
        values += 1;
        bytes = next_bytes;
    }
    projected
}

pub(crate) fn insert_default(headers: &mut HeaderMap, name: &'static str, value: &'static str) {
    if !headers.contains_key(name) {
        headers.insert(
            HeaderName::from_static(name),
            HeaderValue::from_static(value),
        );
    }
}

fn connection_nominated(headers: &HeaderMap) -> Vec<HeaderName> {
    headers
        .get_all(header::CONNECTION)
        .iter()
        .flat_map(|value| value.as_bytes().split(|byte| *byte == b','))
        .filter_map(|name| HeaderName::from_bytes(trim_ows(name)).ok())
        .collect()
}

fn forbidden(name: &str) -> bool {
    matches!(
        name,
        "authorization"
            | "x-api-key"
            | "api-key"
            | "cookie"
            | "set-cookie"
            | "host"
            | "forwarded"
            | "proxy-authenticate"
            | "proxy-authorization"
            | "connection"
            | "keep-alive"
            | "proxy-connection"
            | "te"
            | "trailer"
            | "transfer-encoding"
            | "upgrade"
            | "content-length"
            | "content-encoding"
            | "content-md5"
            | "digest"
            | "etag"
            | "accept-encoding"
            | "baggage"
            | "chatgpt-account-id"
            | "x-userid"
            | "x-xai-token-auth"
            | "x-authenticateresponse"
    ) || name.starts_with("x-forwarded-")
}

fn trim_ows(mut value: &[u8]) -> &[u8] {
    while value
        .first()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[1..];
    }
    while value
        .last()
        .is_some_and(|byte| matches!(byte, b' ' | b'\t'))
    {
        value = &value[..value.len() - 1];
    }
    value
}

#[cfg(test)]
mod tests {
    use http::{HeaderMap, HeaderValue, header};

    use super::project;

    #[test]
    fn projection_never_forwards_secrets_or_connection_nominated_headers() {
        let mut source = HeaderMap::new();
        source.insert("x-api-key", HeaderValue::from_static("secret"));
        source.insert("x-client-request-id", HeaderValue::from_static("client"));
        source.insert("x-private-hop", HeaderValue::from_static("private"));
        source.insert(
            header::CONNECTION,
            HeaderValue::from_static("x-private-hop"),
        );
        let projected = project(
            &source,
            &["x-api-key", "x-client-request-id", "x-private-hop"],
            &[],
        );
        assert_eq!(projected["x-client-request-id"], "client");
        assert!(!projected.contains_key("x-api-key"));
        assert!(!projected.contains_key("x-private-hop"));
    }

    #[test]
    fn projection_enforces_value_count_single_value_and_total_byte_limits() {
        let mut too_many = HeaderMap::new();
        for _ in 0..65 {
            too_many.append("x-safe", HeaderValue::from_static("v"));
        }
        assert_eq!(project(&too_many, &["x-safe"], &[]).len(), 64);

        let mut oversized = HeaderMap::new();
        oversized.insert(
            "x-safe",
            HeaderValue::from_bytes(&vec![b'a'; 8 * 1024 + 1]).expect("valid header value"),
        );
        assert!(project(&oversized, &["x-safe"], &[]).is_empty());

        let mut total = HeaderMap::new();
        for _ in 0..4 {
            total.append(
                "x-safe",
                HeaderValue::from_bytes(&vec![b'a'; 8 * 1024]).expect("valid header value"),
            );
        }
        assert_eq!(project(&total, &["x-safe"], &[]).len(), 3);
    }
}
