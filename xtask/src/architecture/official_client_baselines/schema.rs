use std::collections::BTreeMap;

use anyhow::{Context, Result, bail};
use serde::Deserialize;
use serde_json::Value;
use time::{Date, Month};

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Baseline {
    schema_version: u32,
    provider: String,
    client: Client,
    platform: Platform,
    capture: Capture,
    request: CapturedRequest,
    limitations: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Client {
    product: String,
    entrypoint: String,
    version: String,
    distribution: String,
    executable_sha256: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Platform {
    os: String,
    version: String,
    build: String,
    architecture: String,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Capture {
    date: String,
    operation: String,
    transport: String,
    network_scope: String,
    credential_policy: String,
    environment_policy: String,
    client_home_environment: String,
    explicitly_set_environment: BTreeMap<String, String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapturedRequest {
    request_line: String,
    raw_bytes: u64,
    raw_sha256: String,
    headers: Vec<CapturedHeader>,
    body: CapturedBody,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapturedHeader {
    name: String,
    value: String,
    classification: String,
    #[serde(default)]
    metadata_fields: Vec<String>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CapturedBody {
    bytes: u64,
    sha256: String,
    top_level_fields_in_wire_order: Vec<String>,
    semantics: Value,
    input_items: Vec<Value>,
    client_metadata_fields_in_wire_order: Vec<String>,
    turn_metadata_fields_in_wire_order: Vec<String>,
}

pub(super) fn validate_document(raw: &str) -> Result<()> {
    let baseline: Baseline = serde_json::from_str(raw).context("invalid baseline JSON schema")?;
    validate(&baseline)
}

fn validate(baseline: &Baseline) -> Result<()> {
    if baseline.schema_version != 1 {
        bail!("unsupported schema_version {}", baseline.schema_version);
    }
    require_text("provider", &baseline.provider)?;
    validate_client(&baseline.client)?;
    validate_platform(&baseline.platform)?;
    validate_capture(&baseline.capture)?;
    validate_request(&baseline.request)?;
    if baseline.limitations.is_empty() || baseline.limitations.iter().any(|value| value.is_empty())
    {
        bail!("limitations must contain non-empty entries");
    }
    Ok(())
}

fn validate_client(client: &Client) -> Result<()> {
    for (name, value) in [
        ("client.product", &client.product),
        ("client.entrypoint", &client.entrypoint),
        ("client.version", &client.version),
        ("client.distribution", &client.distribution),
    ] {
        require_text(name, value)?;
    }
    validate_sha256("client.executable_sha256", &client.executable_sha256)
}

fn validate_platform(platform: &Platform) -> Result<()> {
    for (name, value) in [
        ("platform.os", &platform.os),
        ("platform.version", &platform.version),
        ("platform.build", &platform.build),
        ("platform.architecture", &platform.architecture),
    ] {
        require_text(name, value)?;
    }
    Ok(())
}

fn validate_capture(capture: &Capture) -> Result<()> {
    parse_date(&capture.date)?;
    require_text("capture.operation", &capture.operation)?;
    if capture.transport != "loopback_http_1_1"
        || capture.network_scope != "loopback_only"
        || capture.credential_policy != "synthetic"
        || capture.environment_policy != "cleared"
    {
        bail!(
            "capture must use loopback HTTP/1.1, synthetic credentials, and a cleared environment"
        );
    }
    let Some(client_home) = capture
        .explicitly_set_environment
        .get(&capture.client_home_environment)
    else {
        bail!("capture must identify its temporary client home environment variable");
    };
    if client_home != "<temporary-directory>"
        || !capture
            .explicitly_set_environment
            .values()
            .any(|value| value == "<synthetic>")
        || capture
            .explicitly_set_environment
            .contains_key("CODEX_INTERNAL_ORIGINATOR_OVERRIDE")
    {
        bail!("capture environment values are not safely isolated");
    }
    Ok(())
}

fn validate_request(request: &CapturedRequest) -> Result<()> {
    if !request.request_line.ends_with(" HTTP/1.1") {
        bail!("request_line must describe the captured HTTP/1.1 request");
    }
    if request.raw_bytes <= request.body.bytes || request.body.bytes == 0 {
        bail!("request byte counts are inconsistent");
    }
    validate_sha256("request.raw_sha256", &request.raw_sha256)?;
    validate_headers(&request.headers)?;
    validate_body(&request.body)
}

fn validate_headers(headers: &[CapturedHeader]) -> Result<()> {
    if headers.is_empty() {
        bail!("captured headers must not be empty");
    }
    for header in headers {
        if header.name.is_empty() || header.name != header.name.to_ascii_lowercase() {
            bail!("captured Header names must be lowercase");
        }
        if !matches!(
            header.classification.as_str(),
            "replayable" | "credential_owned" | "protocol" | "authentication" | "transport"
        ) {
            bail!("unknown Header classification: {}", header.classification);
        }
        if header.classification == "authentication" && !header.value.starts_with("<redacted:") {
            bail!("authentication Header is not safely redacted");
        }
        if let Some(expected) = match header.name.as_str() {
            "authorization" => Some("<redacted:bearer>"),
            "x-api-key" => Some("<redacted:api-key>"),
            _ => None,
        } && (header.value != expected || header.classification != "authentication")
        {
            bail!("known authentication Header has an invalid classification");
        }
        if header.classification == "credential_owned" && !header.value.starts_with("<dynamic:") {
            bail!("Credential-owned Header is not normalized");
        }
        match header.name.as_str() {
            "host" if header.value != "<loopback-authority>" => {
                bail!("Host must be normalized to loopback authority")
            }
            "content-length" if header.value != "<body-bytes>" => {
                bail!("Content-Length must be normalized to Body bytes")
            }
            _ => {}
        }
        if header.metadata_fields.iter().any(|field| field.is_empty()) {
            bail!("Header metadata field names must not be empty");
        }
    }
    Ok(())
}

fn validate_body(body: &CapturedBody) -> Result<()> {
    validate_sha256("request.body.sha256", &body.sha256)?;
    if body.top_level_fields_in_wire_order.is_empty()
        || body.input_items.is_empty()
        || body.client_metadata_fields_in_wire_order.is_empty()
        || body.turn_metadata_fields_in_wire_order.is_empty()
        || body
            .semantics
            .as_object()
            .is_none_or(serde_json::Map::is_empty)
    {
        bail!("captured Body summary is incomplete");
    }
    Ok(())
}

fn require_text(name: &str, value: &str) -> Result<()> {
    if value.trim().is_empty() {
        bail!("{name} must not be empty");
    }
    Ok(())
}

fn validate_sha256(name: &str, value: &str) -> Result<()> {
    if value.len() != 64
        || !value
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        bail!("{name} must be a lowercase SHA-256 digest");
    }
    Ok(())
}

fn parse_date(value: &str) -> Result<Date> {
    let mut parts = value.split('-');
    let year = parts.next().and_then(|value| value.parse().ok());
    let month = parts.next().and_then(|value| value.parse::<u8>().ok());
    let day = parts.next().and_then(|value| value.parse().ok());
    if parts.next().is_some() {
        bail!("capture.date must use YYYY-MM-DD");
    }
    let month = month.and_then(|value| Month::try_from(value).ok());
    match (year, month, day) {
        (Some(year), Some(month), Some(day)) => {
            Date::from_calendar_date(year, month, day).context("invalid capture.date")
        }
        _ => bail!("capture.date must use YYYY-MM-DD"),
    }
}

#[cfg(test)]
mod tests {
    use super::{CapturedHeader, validate_headers};

    #[test]
    fn rejects_raw_authentication_and_credential_owned_values() {
        for header in [
            CapturedHeader {
                name: "authorization".into(),
                value: "secret".into(),
                classification: "authentication".into(),
                metadata_fields: Vec::new(),
            },
            CapturedHeader {
                name: "session-id".into(),
                value: "raw-session".into(),
                classification: "credential_owned".into(),
                metadata_fields: Vec::new(),
            },
        ] {
            assert!(validate_headers(&[header]).is_err());
        }
    }
}
