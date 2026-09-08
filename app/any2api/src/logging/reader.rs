use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, BufRead, BufReader, Read, Seek, SeekFrom},
    path::{Path, PathBuf},
};

use any2api_server::api::{
    RuntimeLogEntry, RuntimeLogLevel, RuntimeLogPage, RuntimeLogQuery, RuntimeLogSource,
    runtime_log_summary,
};
use async_trait::async_trait;
use serde_json::Value;
use time::{OffsetDateTime, format_description::well_known::Rfc3339};

use super::file::{is_managed_file, managed_files};

const PAGE_SIZE: usize = 100;
const READ_WINDOW: u64 = 256 * 1024;
const MAX_SCAN_BYTES: u64 = 4 * 1024 * 1024;

pub(crate) struct FileLogReader {
    directory: PathBuf,
}

impl FileLogReader {
    pub(crate) fn new(directory: PathBuf) -> Self {
        Self { directory }
    }
}

#[async_trait]
impl RuntimeLogSource for FileLogReader {
    async fn list(&self, query: RuntimeLogQuery) -> io::Result<RuntimeLogPage> {
        let directory = self.directory.clone();
        tokio::task::spawn_blocking(move || read_page(&directory, &query))
            .await
            .map_err(io::Error::other)?
    }
}

fn read_page(directory: &Path, query: &RuntimeLogQuery) -> io::Result<RuntimeLogPage> {
    let cursor = query.cursor.as_deref().map(parse_cursor).transpose()?;
    let mut files = match managed_files(directory, None) {
        Ok(files) => files,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            return Ok(RuntimeLogPage::default());
        }
        Err(error) => return Err(error),
    };
    // Segment numbers can be reused after retention. The first event remains a
    // stable ordering key while the current segment is appended to.
    let mut ordered = Vec::with_capacity(files.len());
    for segment in files.drain(..) {
        let file = match File::open(&segment.path) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let mut first = Vec::new();
        BufReader::new(file.take(READ_WINDOW)).read_until(b'\n', &mut first)?;
        let started = serde_json::from_slice::<Value>(&first)
            .ok()
            .and_then(|value| {
                OffsetDateTime::parse(value.get("timestamp")?.as_str()?, &Rfc3339).ok()
            })
            .map_or_else(
                || {
                    segment
                        .modified
                        .duration_since(std::time::UNIX_EPOCH)
                        .unwrap_or_default()
                        .as_nanos() as i128
                },
                |time| time.unix_timestamp_nanos(),
            );
        ordered.push((started, segment));
    }
    ordered.sort_unstable_by(|a, b| (b.0, &b.1.path).cmp(&(a.0, &a.1.path)));
    let mut page = RuntimeLogPage::default();
    let mut scanned = 0;
    let search = query
        .search
        .as_deref()
        .unwrap_or_default()
        .trim()
        .to_lowercase();
    for (index, (started, segment)) in ordered.iter().enumerate() {
        let name = segment
            .path
            .file_name()
            .and_then(|name| name.to_str())
            .expect("managed log name");
        if cursor.is_some_and(|(time, file, _)| (*started, name) > (time, file)) {
            continue;
        }
        let mut file = match File::open(&segment.path) {
            Ok(file) => file,
            // Retention can remove a segment after it was listed.
            Err(error) if error.kind() == io::ErrorKind::NotFound => continue,
            Err(error) => return Err(error),
        };
        let size = file.metadata()?.len();
        let mut end = cursor
            .filter(|(time, file, _)| *time == *started && *file == name)
            .map_or(size, |(_, _, offset)| offset.min(size));
        while end > 0 {
            let start = end.saturating_sub(READ_WINDOW);
            file.seek(SeekFrom::Start(start))?;
            let mut bytes = vec![0; (end - start) as usize];
            file.read_exact(&mut bytes)?;
            scanned += end - start;
            // Keep a record crossing the window boundary for the next read.
            let first = if start == 0 {
                0
            } else {
                bytes
                    .iter()
                    .position(|byte| *byte == b'\n')
                    .map_or(bytes.len(), |at| at + 1)
            };
            let mut offset = end;
            for line in bytes[first..].split_inclusive(|byte| *byte == b'\n').rev() {
                offset -= line.len() as u64;
                // The active writer may still be appending its last event.
                if line.last() != Some(&b'\n') {
                    continue;
                }
                if let Some(entry) = parse_entry(line, format!("{started}:{name}:{offset}"))
                    && matches_filter(&entry, query, &search)
                {
                    page.items.push(entry);
                    if page.items.len() == PAGE_SIZE {
                        page.next_cursor =
                            next_cursor(*started, name, offset, index + 1 < ordered.len());
                        return Ok(page);
                    }
                }
            }
            let boundary = start + first as u64;
            end = if boundary < end { boundary } else { start };
            if scanned >= MAX_SCAN_BYTES {
                page.next_cursor = next_cursor(*started, name, end, index + 1 < ordered.len());
                return Ok(page);
            }
        }
    }
    Ok(page)
}

fn next_cursor(started: i128, file: &str, offset: u64, has_older_files: bool) -> Option<String> {
    (offset > 0 || has_older_files).then(|| format!("{started}:{file}:{offset}"))
}

fn parse_cursor(value: &str) -> io::Result<(i128, &str, u64)> {
    let parsed = value.rsplit_once(':').and_then(|(key, offset)| {
        let (started, name) = key.split_once(':')?;
        let path = Path::new(name);
        (path.file_name()?.to_str()? == name && is_managed_file(path)).then_some((
            started.parse().ok()?,
            name,
            offset.parse::<u64>().ok()?,
        ))
    });
    parsed.ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, "invalid runtime log cursor"))
}

fn parse_entry(line: &[u8], id: String) -> Option<RuntimeLogEntry> {
    let value: Value = serde_json::from_slice(line).ok()?;
    let target = value.get("target")?.as_str()?;
    if target != "any2api" && !target.starts_with("any2api::") && !target.starts_with("any2api_") {
        return None;
    }
    let level = match value.get("level")?.as_str()? {
        "ERROR" => RuntimeLogLevel::Error,
        "WARN" => RuntimeLogLevel::Warn,
        "INFO" => RuntimeLogLevel::Info,
        "DEBUG" => RuntimeLogLevel::Debug,
        "TRACE" => RuntimeLogLevel::Trace,
        _ => return None,
    };
    let mut fields = BTreeMap::new();
    // Project diagnostic fields instead of exposing arbitrary tracing payloads.
    for key in [
        "event",
        "error",
        "oauth_account_id",
        "provider",
        "request_id",
        "refresh_trigger",
        "refresh_stage",
        "refresh_reason",
        "upstream_status",
        "failure_scope",
        "reauthorization_required",
        "token_version",
        "occurred_at",
        "records",
        "prepared_account_count",
        "config_revision",
        "address",
    ] {
        if let Some(value) = value.get(key).filter(|value| !value.is_null()) {
            let text = match value {
                Value::String(value) => value.clone(),
                Value::Bool(_) | Value::Number(_) => value.to_string(),
                _ => continue,
            };
            fields.insert(key.to_owned(), text);
        }
    }
    let message = value.get("message")?.as_str()?;
    Some(RuntimeLogEntry {
        id,
        timestamp: value.get("timestamp")?.as_str()?.to_owned(),
        level,
        module: module_name(target).to_owned(),
        target: target.to_owned(),
        message: message.to_owned(),
        summary: runtime_log_summary(message).to_owned(),
        fields,
    })
}

fn module_name(target: &str) -> &str {
    if target.contains("::oauth") {
        "oauth"
    } else if target.contains("::configuration") {
        "configuration"
    } else if target.starts_with("any2api_storage") {
        "storage"
    } else if target.contains("telemetry")
        || target.contains("::logging")
        || target.contains("http_access_log")
    {
        "logging"
    } else if target.contains("official_client_version") {
        "client_version"
    } else if target.starts_with("any2api_updater") || target.contains("self_update") {
        "update"
    } else if target.starts_with("any2api_server") {
        "http"
    } else if target.starts_with("any2api::") || target == "any2api" {
        "application"
    } else {
        "runtime"
    }
}

fn matches_filter(entry: &RuntimeLogEntry, query: &RuntimeLogQuery, search: &str) -> bool {
    query.level.is_none_or(|level| entry.level == level)
        && query
            .module
            .as_deref()
            .is_none_or(|module| entry.module == module)
        && (search.is_empty()
            || entry.message.to_lowercase().contains(search)
            || entry.summary.to_lowercase().contains(search)
            || entry.target.to_lowercase().contains(search)
            || entry
                .fields
                .values()
                .any(|value| value.to_lowercase().contains(search)))
}

#[cfg(test)]
mod tests;
