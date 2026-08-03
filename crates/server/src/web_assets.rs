use std::path::PathBuf;

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
