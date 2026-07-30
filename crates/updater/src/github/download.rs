use std::{path::Path, time::Duration};

use reqwest::{Client, Response, redirect};
use sha2::{Digest, Sha256};
use tokio::{fs::File, io::AsyncWriteExt};

use crate::api::{UpdateError, UpdateErrorKind};

use super::{
    LATEST_RELEASE_API, MAX_ARCHIVE_BYTES, MAX_CHECKSUM_BYTES, MAX_METADATA_BYTES, Release,
    parse_release,
};

pub(crate) fn client() -> Result<Client, UpdateError> {
    Client::builder()
        .user_agent(format!("any2api/{}", crate::BUILD_VERSION))
        .https_only(true)
        .connect_timeout(Duration::from_secs(10))
        .redirect(redirect::Policy::custom(|attempt| {
            let allowed = attempt.url().scheme() == "https"
                && attempt.url().host_str().is_some_and(is_allowed_github_host);
            if attempt.previous().len() >= 5 {
                attempt.error("too many release download redirects")
            } else if allowed {
                attempt.follow()
            } else {
                attempt.stop()
            }
        }))
        .build()
        .map_err(|error| {
            UpdateError::new(
                UpdateErrorKind::CheckFailed,
                format!("failed to initialize GitHub client: {error}"),
            )
        })
}

pub(crate) async fn latest(client: &Client) -> Result<Release, UpdateError> {
    let response = client
        .get(LATEST_RELEASE_API)
        .header("Accept", "application/vnd.github+json")
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| check_failed(format!("GitHub release request failed: {error}")))?;
    ensure_success(&response, UpdateErrorKind::CheckFailed)?;
    let bytes = read_bounded(response, MAX_METADATA_BYTES, UpdateErrorKind::CheckFailed).await?;
    parse_release(&bytes)
}

pub(crate) async fn download_checksum(
    client: &Client,
    release: &Release,
) -> Result<Vec<u8>, UpdateError> {
    let response = client
        .get(release.checksum_url())
        .timeout(Duration::from_secs(30))
        .send()
        .await
        .map_err(|error| download_failed(format!("checksum download failed: {error}")))?;
    ensure_success(&response, UpdateErrorKind::DownloadFailed)?;
    let bytes = read_bounded(
        response,
        MAX_CHECKSUM_BYTES,
        UpdateErrorKind::DownloadFailed,
    )
    .await?;
    ensure_exact_size(bytes.len(), release.checksum_size, "checksum")?;
    Ok(bytes)
}

pub(crate) async fn download_archive<Progress>(
    client: &Client,
    release: &Release,
    path: &Path,
    mut progress: Progress,
) -> Result<String, UpdateError>
where
    Progress: FnMut(u64) + Send,
{
    let mut response = client
        .get(release.archive_url())
        .timeout(Duration::from_secs(300))
        .send()
        .await
        .map_err(|error| download_failed(format!("release download failed: {error}")))?;
    ensure_success(&response, UpdateErrorKind::DownloadFailed)?;
    reject_large_content_length(
        &response,
        MAX_ARCHIVE_BYTES,
        UpdateErrorKind::DownloadFailed,
    )?;

    let mut file = File::create(path)
        .await
        .map_err(|error| download_failed(format!("failed to create update archive: {error}")))?;
    let mut size = 0_u64;
    let mut digest = Sha256::new();
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| download_failed(format!("release download failed: {error}")))?
    {
        size = size
            .checked_add(u64::try_from(chunk.len()).unwrap_or(u64::MAX))
            .ok_or_else(|| download_failed("release archive size overflow"))?;
        if size > MAX_ARCHIVE_BYTES {
            return Err(download_failed("release archive exceeds the size limit"));
        }
        digest.update(&chunk);
        file.write_all(&chunk)
            .await
            .map_err(|error| download_failed(format!("failed to write update archive: {error}")))?;
        progress(size);
    }
    ensure_exact_size_u64(size, release.archive_size, "release archive")?;
    file.sync_all()
        .await
        .map_err(|error| download_failed(format!("failed to sync update archive: {error}")))?;
    Ok(format!("{:x}", digest.finalize()))
}

async fn read_bounded(
    mut response: Response,
    limit: u64,
    kind: UpdateErrorKind,
) -> Result<Vec<u8>, UpdateError> {
    reject_large_content_length(&response, limit, kind)?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.map_err(|error| {
        UpdateError::new(kind, format!("failed to read GitHub response: {error}"))
    })? {
        if bytes.len().saturating_add(chunk.len()) > usize::try_from(limit).unwrap_or(usize::MAX) {
            return Err(UpdateError::new(
                kind,
                "GitHub response exceeds the size limit",
            ));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

fn ensure_success(response: &Response, kind: UpdateErrorKind) -> Result<(), UpdateError> {
    if response.status().is_success() {
        return Ok(());
    }
    Err(UpdateError::new(
        kind,
        format!("GitHub returned HTTP {}", response.status()),
    ))
}

fn reject_large_content_length(
    response: &Response,
    limit: u64,
    kind: UpdateErrorKind,
) -> Result<(), UpdateError> {
    if response
        .content_length()
        .is_some_and(|length| length > limit)
    {
        return Err(UpdateError::new(
            kind,
            "GitHub response exceeds the size limit",
        ));
    }
    Ok(())
}

fn ensure_exact_size(actual: usize, expected: u64, label: &str) -> Result<(), UpdateError> {
    ensure_exact_size_u64(u64::try_from(actual).unwrap_or(u64::MAX), expected, label)
}

fn ensure_exact_size_u64(actual: u64, expected: u64, label: &str) -> Result<(), UpdateError> {
    if actual != expected {
        return Err(download_failed(format!(
            "{label} size does not match GitHub metadata"
        )));
    }
    Ok(())
}

fn is_allowed_github_host(host: &str) -> bool {
    host == "github.com"
        || host.ends_with(".github.com")
        || host == "githubusercontent.com"
        || host.ends_with(".githubusercontent.com")
}

fn check_failed(message: impl Into<String>) -> UpdateError {
    UpdateError::new(UpdateErrorKind::CheckFailed, message)
}

fn download_failed(message: impl Into<String>) -> UpdateError {
    UpdateError::new(UpdateErrorKind::DownloadFailed, message)
}
