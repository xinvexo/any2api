use std::path::PathBuf;

pub(crate) const MANAGEMENT_DEEP_LINKS: &[&str] = &[
    "/",
    "/proxies",
    "/providers",
    "/oauth",
    "/routes",
    "/quota-rates",
    "/keys",
    "/logs",
    "/logs/{request_id}",
    "/system-logs",
    "/settings",
    "/settings/{section}",
];

pub(crate) fn is_management_deep_link(path: &str) -> bool {
    MANAGEMENT_DEEP_LINKS
        .iter()
        .any(|pattern| route_pattern_matches(pattern, path))
}

fn route_pattern_matches(pattern: &str, path: &str) -> bool {
    if pattern == "/" {
        return path == "/";
    }

    let mut expected = pattern.trim_matches('/').split('/');
    let mut actual = path.trim_matches('/').split('/');
    loop {
        match (expected.next(), actual.next()) {
            (None, None) => return true,
            (Some(expected), Some(actual))
                if expected == actual
                    || (!actual.is_empty()
                        && expected.starts_with('{')
                        && expected.ends_with('}')) => {}
            _ => return false,
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub struct EmbeddedWebAsset {
    path: &'static str,
    bytes: &'static [u8],
    etag: &'static str,
}

impl EmbeddedWebAsset {
    #[must_use]
    pub const fn new(path: &'static str, bytes: &'static [u8], etag: &'static str) -> Self {
        Self { path, bytes, etag }
    }

    pub(crate) const fn path(self) -> &'static str {
        self.path
    }

    pub(crate) const fn bytes(self) -> &'static [u8] {
        self.bytes
    }

    pub(crate) const fn etag(self) -> &'static str {
        self.etag
    }
}

#[derive(Debug)]
pub enum WebAssets {
    External(PathBuf),
    Embedded(&'static [EmbeddedWebAsset]),
}

impl WebAssets {
    #[must_use]
    pub fn external(root: impl Into<PathBuf>) -> Self {
        Self::External(root.into())
    }

    #[must_use]
    pub const fn embedded(assets: &'static [EmbeddedWebAsset]) -> Self {
        Self::Embedded(assets)
    }
}

impl From<PathBuf> for WebAssets {
    fn from(root: PathBuf) -> Self {
        Self::External(root)
    }
}
