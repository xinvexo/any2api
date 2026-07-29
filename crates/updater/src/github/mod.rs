mod download;

use semver::Version;
use serde::Deserialize;

use crate::api::{UpdateError, UpdateErrorKind};

pub(crate) use download::{client, download_archive, download_checksum, latest};

pub(crate) const REPOSITORY_URL: &str = "https://github.com/xinvexo/any2api";
const LATEST_RELEASE_API: &str = "https://api.github.com/repos/xinvexo/any2api/releases/latest";
const MAX_METADATA_BYTES: u64 = 1024 * 1024;
pub(crate) const MAX_ARCHIVE_BYTES: u64 = 128 * 1024 * 1024;
const MAX_CHECKSUM_BYTES: u64 = 4096;

#[derive(Clone, Debug)]
pub(crate) struct Release {
    pub(crate) version: Version,
    pub(crate) release_url: String,
    pub(crate) published_at: Option<String>,
    pub(crate) archive_name: String,
    pub(crate) archive_size: u64,
    pub(crate) checksum_name: String,
    pub(crate) checksum_size: u64,
}

impl Release {
    fn asset_url(&self, name: &str) -> String {
        format!(
            "{REPOSITORY_URL}/releases/download/v{}/{name}",
            self.version
        )
    }

    fn archive_url(&self) -> String {
        self.asset_url(&self.archive_name)
    }

    fn checksum_url(&self) -> String {
        self.asset_url(&self.checksum_name)
    }
}

#[derive(Deserialize)]
struct ReleaseDocument {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    published_at: Option<String>,
    assets: Vec<ReleaseAsset>,
}

#[derive(Deserialize)]
struct ReleaseAsset {
    name: String,
    size: u64,
}

fn parse_release(bytes: &[u8]) -> Result<Release, UpdateError> {
    let document: ReleaseDocument = serde_json::from_slice(bytes).map_err(|error| {
        UpdateError::new(
            UpdateErrorKind::InvalidRelease,
            format!("GitHub returned invalid release metadata: {error}"),
        )
    })?;
    if document.draft || document.prerelease {
        return Err(invalid_release(
            "latest release is not a stable published release",
        ));
    }
    let raw_version = document
        .tag_name
        .strip_prefix('v')
        .ok_or_else(|| invalid_release("release tag must use the v<SemVer> format"))?;
    let version = Version::parse(raw_version)
        .map_err(|_| invalid_release("release tag must contain a valid SemVer"))?;
    if !version.pre.is_empty() || !version.build.is_empty() {
        return Err(invalid_release("release tag must be a stable plain SemVer"));
    }

    let archive_name = format!("any2api-v{version}-linux-amd64.tar.gz");
    let checksum_name = format!("{archive_name}.sha256");
    let archive_size = unique_asset_size(&document.assets, &archive_name, MAX_ARCHIVE_BYTES)?;
    let checksum_size = unique_asset_size(&document.assets, &checksum_name, MAX_CHECKSUM_BYTES)?;
    Ok(Release {
        release_url: format!("{REPOSITORY_URL}/releases/tag/v{version}"),
        published_at: document.published_at,
        archive_name,
        archive_size,
        checksum_name,
        checksum_size,
        version,
    })
}

fn unique_asset_size(
    assets: &[ReleaseAsset],
    expected_name: &str,
    max_size: u64,
) -> Result<u64, UpdateError> {
    let mut matching = assets.iter().filter(|asset| asset.name == expected_name);
    let size = matching
        .next()
        .ok_or_else(|| invalid_release(format!("release is missing {expected_name}")))?
        .size;
    if matching.next().is_some() || size == 0 || size > max_size {
        return Err(invalid_release(format!(
            "release asset {expected_name} has an invalid size or duplicate"
        )));
    }
    Ok(size)
}

fn invalid_release(message: impl Into<String>) -> UpdateError {
    UpdateError::new(UpdateErrorKind::InvalidRelease, message)
}

#[cfg(test)]
pub(crate) fn parse_release_for_test(bytes: &[u8]) -> Result<Release, UpdateError> {
    parse_release(bytes)
}
