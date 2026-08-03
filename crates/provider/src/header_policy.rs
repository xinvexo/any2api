use http::{HeaderMap, HeaderName, HeaderValue, header};

const MAX_FORWARDED_HEADER_VALUES: usize = 64;
const MAX_FORWARDED_HEADER_VALUE_BYTES: usize = 8 * 1024;
const MAX_FORWARDED_HEADER_BYTES: usize = 32 * 1024;

pub(crate) fn project(source: &HeaderMap, exact: &[&str], prefixes: &[&str]) -> HeaderMap {
    let connection_nominated = connection_nominated(source);
    let exact = ordered_exact_names(exact);
    let mut projected = HeaderMap::new();
    let mut values = 0_usize;
    let mut bytes = 0_usize;
    for name in &exact {
        if !append_name(
            source,
            name,
            &connection_nominated,
            &mut projected,
            &mut values,
            &mut bytes,
        ) {
            return projected;
        }
    }
    for (_, name) in ordered_prefix_names(source, &exact, prefixes) {
        if !append_name(
            source,
            &name,
            &connection_nominated,
            &mut projected,
            &mut values,
            &mut bytes,
        ) {
            return projected;
        }
    }
    projected
}

fn ordered_exact_names(exact: &[&str]) -> Vec<HeaderName> {
    let mut names = Vec::with_capacity(exact.len());
    for value in exact {
        let Ok(name) = HeaderName::from_bytes(value.as_bytes()) else {
            continue;
        };
        if !names.contains(&name) {
            names.push(name);
        }
    }
    names
}

fn ordered_prefix_names(
    source: &HeaderMap,
    exact: &[HeaderName],
    prefixes: &[&str],
) -> Vec<(usize, HeaderName)> {
    let mut names = source
        .keys()
        .filter(|name| !exact.contains(name))
        .filter_map(|name| {
            prefixes
                .iter()
                .position(|prefix| name.as_str().starts_with(prefix))
                .map(|priority| (priority, name.clone()))
        })
        .collect::<Vec<_>>();
    names.sort_unstable_by(|(left_priority, left), (right_priority, right)| {
        left_priority
            .cmp(right_priority)
            .then_with(|| left.as_str().cmp(right.as_str()))
    });
    names
}

fn append_name(
    source: &HeaderMap,
    name: &HeaderName,
    connection_nominated: &[HeaderName],
    projected: &mut HeaderMap,
    values: &mut usize,
    bytes: &mut usize,
) -> bool {
    if forbidden(name.as_str()) || connection_nominated.contains(name) {
        return true;
    }
    for value in source.get_all(name).iter() {
        if value.as_bytes().len() > MAX_FORWARDED_HEADER_VALUE_BYTES {
            continue;
        }
        if *values >= MAX_FORWARDED_HEADER_VALUES {
            return false;
        }
        let next_bytes = bytes
            .saturating_add(name.as_str().len())
            .saturating_add(value.as_bytes().len());
        if next_bytes > MAX_FORWARDED_HEADER_BYTES {
            continue;
        }
        projected.append(name.clone(), value.clone());
        *values += 1;
        *bytes = next_bytes;
    }
    true
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
        source.insert("content-encoding", HeaderValue::from_static("gzip"));
        source.insert("x-private-hop", HeaderValue::from_static("private"));
        source.insert(
            header::CONNECTION,
            HeaderValue::from_static("x-private-hop"),
        );
        let projected = project(
            &source,
            &[
                "content-encoding",
                "x-api-key",
                "x-client-request-id",
                "x-private-hop",
            ],
            &[],
        );
        assert_eq!(projected["x-client-request-id"], "client");
        assert_eq!(projected["content-encoding"], "gzip");
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
        total.insert("x-small", HeaderValue::from_static("fits"));
        let projected = project(&total, &["x-safe", "x-small"], &[]);
        assert_eq!(projected.get_all("x-safe").iter().count(), 3);
        assert_eq!(projected["x-small"], "fits");
    }

    #[test]
    fn exact_priority_is_independent_of_source_insertion_order() {
        let projected = [true, false].map(|bulk_first| {
            let mut source = HeaderMap::new();
            if !bulk_first {
                source.insert("x-important", HeaderValue::from_static("keep"));
            }
            for _ in 0..64 {
                source.append("x-bulk", HeaderValue::from_static("v"));
            }
            if bulk_first {
                source.insert("x-important", HeaderValue::from_static("keep"));
            }
            project(&source, &["x-important", "x-bulk"], &[])
        });

        for headers in projected {
            assert_eq!(headers["x-important"], "keep");
            assert_eq!(headers.get_all("x-bulk").iter().count(), 63);
        }
    }

    #[test]
    fn prefix_priority_is_declared_then_lexical() {
        let mut source = HeaderMap::new();
        for _ in 0..63 {
            source.append("x-z-last", HeaderValue::from_static("v"));
        }
        source.insert("x-a-first", HeaderValue::from_static("a"));
        source.insert("y-declared-first", HeaderValue::from_static("y"));

        let projected = project(&source, &[], &["y-", "x-"]);

        assert_eq!(projected["y-declared-first"], "y");
        assert_eq!(projected["x-a-first"], "a");
        assert_eq!(projected.get_all("x-z-last").iter().count(), 62);
    }
}
