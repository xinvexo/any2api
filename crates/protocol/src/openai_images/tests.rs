use std::convert::Infallible;

use any2api_domain::{ProtocolOperation, PublicError, PublicErrorCode, TokenUsage};
use bytes::Bytes;
use futures_util::stream;
use http::{HeaderMap, HeaderValue, Method, StatusCode, Uri, header};
use multer::Multipart;
use serde_json::{Value, json};

use super::OpenAiImagesAdapter;
use crate::api::{IngressAffinity, IngressRequest, ProtocolAdapter, SseFrame, UpstreamResponse};

#[tokio::test]
async fn generation_and_json_edit_preserve_unknown_fields_and_rewrite_model() {
    let adapter = OpenAiImagesAdapter::new();
    for (operation, uri, body) in [
        (
            ProtocolOperation::ImagesGenerations,
            "/v1/images/generations",
            json!({
                "model": "gpt-image-2",
                "prompt": "a lighthouse",
                "stream": true,
                "future_generation_option": {"kept": true}
            }),
        ),
        (
            ProtocolOperation::ImagesEdits,
            "/v1/images/edits",
            json!({
                "model": "gpt-image-2",
                "prompt": "add fog",
                "images": [{"image_url": "https://example.com/source.png"}],
                "mask": {"image_url": "https://example.com/mask.png"},
                "future_edit_option": 42
            }),
        ),
    ] {
        let decoded = adapter
            .decode_ingress_request(json_request(operation, uri, body))
            .await
            .expect("Images JSON request decodes");
        assert_eq!(decoded.model.as_deref(), Some("gpt-image-2"));
        let encoded = adapter
            .encode_upstream_request(
                decoded.operation,
                decoded.headers,
                decoded.payload,
                "upstream-image-model",
            )
            .expect("Images JSON request encodes");
        let encoded_body: Value = serde_json::from_slice(&encoded.body).expect("encoded JSON");
        assert_eq!(encoded_body["model"], "upstream-image-model");
        if operation == ProtocolOperation::ImagesGenerations {
            assert_eq!(encoded.headers[header::ACCEPT], "text/event-stream");
            assert_eq!(encoded_body["future_generation_option"]["kept"], true);
        } else {
            assert_eq!(encoded.headers[header::ACCEPT], "application/json");
            assert_eq!(encoded_body["future_edit_option"], 42);
            assert_eq!(
                encoded_body["images"][0]["image_url"],
                "https://example.com/source.png"
            );
        }
    }
}

#[tokio::test]
async fn multipart_edit_reencodes_ordered_files_headers_and_replacement_model() {
    let adapter = OpenAiImagesAdapter::new();
    let boundary = "test-boundary";
    let image_one = b"\x89PNG\r\nfirst\0image";
    let image_two = b"\xff\xd8\xffsecond-image";
    let body = multipart_body(
        boundary,
        &[
            Part::text("model", b"gpt-image-2"),
            Part::file("image[]", "first.png", "image/png", image_one)
                .with_header("x-part-meta", "first")
                .with_header("authorization", "must-not-forward"),
            Part::text("future_field", b"preserved"),
            Part::file("image[]", "second.jpg", "image/jpeg", image_two),
            Part::text("stream", b"true"),
        ],
    );
    let decoded = adapter
        .decode_ingress_request(multipart_request(boundary, body))
        .await
        .expect("multipart edit decodes");
    assert_eq!(decoded.model.as_deref(), Some("gpt-image-2"));
    assert!(decoded.stream);

    let encoded = adapter
        .encode_upstream_request(
            decoded.operation,
            decoded.headers,
            decoded.payload,
            "upstream-image-model",
        )
        .expect("multipart edit encodes");
    assert_eq!(encoded.headers[header::ACCEPT], "text/event-stream");
    let content_type = encoded.headers[header::CONTENT_TYPE]
        .to_str()
        .expect("content type");
    let fields = read_fields(encoded.body, content_type).await;
    assert_eq!(
        fields
            .iter()
            .map(|field| field.name.as_str())
            .collect::<Vec<_>>(),
        ["model", "image[]", "future_field", "image[]", "stream"]
    );
    assert_eq!(fields[0].body, Bytes::from_static(b"upstream-image-model"));
    assert_eq!(fields[1].body, Bytes::from_static(image_one));
    assert_eq!(fields[2].body, Bytes::from_static(b"preserved"));
    assert_eq!(fields[3].body, Bytes::from_static(image_two));
    assert_eq!(fields[1].headers["x-part-meta"], "first");
    assert!(!fields[1].headers.contains_key(header::AUTHORIZATION));
}

#[tokio::test]
async fn multipart_edit_rejects_invalid_boundary_model_and_stream() {
    let adapter = OpenAiImagesAdapter::new();
    let invalid_boundary = IngressRequest {
        method: Method::POST,
        uri: Uri::from_static("/v1/images/edits"),
        headers: HeaderMap::from_iter([(
            header::CONTENT_TYPE,
            HeaderValue::from_static("multipart/form-data"),
        )]),
        body: Bytes::from_static(b"not multipart"),
        operation: ProtocolOperation::ImagesEdits,
    };
    assert!(
        adapter
            .decode_ingress_request(invalid_boundary)
            .await
            .is_err()
    );

    for parts in [
        vec![Part::text("prompt", b"missing model")],
        vec![
            Part::text("model", b"gpt-image-2"),
            Part::text("model", b"duplicate"),
        ],
        vec![
            Part::text("model", b"gpt-image-2"),
            Part::text("stream", b"yes"),
        ],
        vec![
            Part::text("model", b"gpt-image-2"),
            Part::text("stream", b"true"),
            Part::text("stream", b"false"),
        ],
        vec![Part::text("model", b"   ")],
    ] {
        let request =
            multipart_request("invalid-payload", multipart_body("invalid-payload", &parts));
        assert!(adapter.decode_ingress_request(request).await.is_err());
    }
}

#[tokio::test]
async fn image_json_and_multipart_requests_ignore_session_identifiers() {
    let adapter = OpenAiImagesAdapter::new();
    for (operation, uri) in [
        (
            ProtocolOperation::ImagesGenerations,
            "/v1/images/generations",
        ),
        (ProtocolOperation::ImagesEdits, "/v1/images/edits"),
    ] {
        let mut request = json_request(
            operation,
            uri,
            json!({
                "model": "gpt-image-2",
                "prompt": "stateless image request",
                "conversation_id": "must-be-ignored"
            }),
        );
        request.headers.insert(
            "x-any2api-session",
            HeaderValue::from_static("must-be-ignored"),
        );
        let decoded = adapter
            .decode_ingress_request(request)
            .await
            .expect("Images JSON request decodes");
        assert_eq!(decoded.affinity, IngressAffinity::None);
    }

    let boundary = "stateless-multipart";
    let body = multipart_body(
        boundary,
        &[
            Part::text("model", b"gpt-image-2"),
            Part::file("image", "source.png", "image/png", b"image-bytes"),
        ],
    );
    let mut request = multipart_request(boundary, body);
    request.headers.insert(
        "x-any2api-session",
        HeaderValue::from_static("must-be-ignored"),
    );
    let decoded = adapter
        .decode_ingress_request(request)
        .await
        .expect("Images multipart request decodes");
    assert_eq!(decoded.affinity, IngressAffinity::None);
}

#[test]
fn image_json_and_completion_events_report_usage_without_content_deltas() {
    let adapter = OpenAiImagesAdapter::new();
    let body = Bytes::from_static(
        br#"{"created":1,"data":[{"b64_json":"abc"}],"usage":{"input_tokens":12,"output_tokens":7,"total_tokens":19}}"#,
    );
    let response = adapter
        .decode_upstream_response(UpstreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: body.clone(),
        })
        .expect("Images response decodes");
    assert_eq!(
        response.telemetry.token_usage,
        TokenUsage::new(Some(12), Some(7), None)
    );
    let egress = adapter
        .encode_egress_response(response, "public-image-model")
        .expect("Images response encodes");
    assert_eq!(egress.body, body);

    for kind in ["image_generation.completed", "image_edit.completed"] {
        let bytes = Bytes::from(format!(
            "event: {kind}\ndata: {{\"type\":\"{kind}\",\"b64_json\":\"abc\",\"usage\":{{\"input_tokens\":12,\"output_tokens\":7}}}}\n\n"
        ));
        let event = adapter
            .decode_upstream_event(SseFrame(bytes.clone()))
            .expect("completion event decodes");
        assert_eq!(
            event.telemetry().token_usage,
            TokenUsage::new(Some(12), Some(7), None)
        );
        assert!(!event.telemetry().has_content_delta);
        assert_eq!(
            adapter
                .encode_egress_event(event, "public-image-model")
                .expect("completion event encodes")
                .0,
            bytes
        );
    }

    let partial = adapter
        .decode_upstream_event(SseFrame(Bytes::from_static(
            b"event: image_generation.partial_image\ndata: {\"type\":\"image_generation.partial_image\",\"b64_json\":\"abc\"}\n\n",
        )))
        .expect("partial event decodes");
    assert_eq!(partial.telemetry().token_usage, TokenUsage::default());
    assert!(!partial.telemetry().has_content_delta);
}

#[test]
fn image_json_and_sse_restore_the_public_model_only_in_known_fields() {
    let adapter = OpenAiImagesAdapter::new();
    let response = adapter
        .decode_upstream_response(UpstreamResponse {
            status: StatusCode::OK,
            headers: HeaderMap::new(),
            body: Bytes::from_static(
                br#"{"model":"upstream-image-model","data":[{"b64_json":"abc"}],"metadata":{"model":"keep"}}"#,
            ),
        })
        .expect("Images response decodes");
    let egress = adapter
        .encode_egress_response(response, "public-image-model")
        .expect("Images response encodes");
    let body: Value = serde_json::from_slice(&egress.body).expect("Images response JSON");
    assert_eq!(body["model"], "public-image-model");
    assert_eq!(body["metadata"]["model"], "keep");

    let event = adapter
        .decode_upstream_event(SseFrame(Bytes::from_static(
            b"event: image_generation.completed\ndata: {\"type\":\"image_generation.completed\",\"model\":\"upstream-image-model\",\"metadata\":{\"model\":\"keep\"}}\n\n",
        )))
        .expect("Images event decodes");
    let frame = adapter
        .encode_egress_event(event, "public-image-model")
        .expect("Images event encodes");
    let text = std::str::from_utf8(&frame.0).expect("Images SSE UTF-8");
    let data = text
        .lines()
        .find_map(|line| line.strip_prefix("data: "))
        .expect("Images SSE data");
    let value: Value = serde_json::from_str(data).expect("Images SSE JSON");
    assert_eq!(value["model"], "public-image-model");
    assert_eq!(value["metadata"]["model"], "keep");
}

#[test]
fn local_image_errors_use_the_openai_error_envelope() {
    let response = OpenAiImagesAdapter::new().error_response(&PublicError::new(
        PublicErrorCode::ModelNotFound,
        "missing image model",
    ));
    let body: Value = serde_json::from_slice(&response.body).expect("error JSON");
    assert_eq!(response.status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["type"], "invalid_request_error");
    assert_eq!(body["error"]["code"], "model_not_found");
}

fn json_request(operation: ProtocolOperation, uri: &'static str, body: Value) -> IngressRequest {
    IngressRequest {
        method: Method::POST,
        uri: Uri::from_static(uri),
        headers: HeaderMap::new(),
        body: Bytes::from(serde_json::to_vec(&body).expect("request JSON")),
        operation,
    }
}

fn multipart_request(boundary: &str, body: Bytes) -> IngressRequest {
    let mut headers = HeaderMap::new();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_str(&format!("multipart/form-data; boundary={boundary}"))
            .expect("multipart content type"),
    );
    IngressRequest {
        method: Method::POST,
        uri: Uri::from_static("/v1/images/edits"),
        headers,
        body,
        operation: ProtocolOperation::ImagesEdits,
    }
}

#[derive(Clone)]
struct Part<'a> {
    name: &'a str,
    file_name: Option<&'a str>,
    content_type: Option<&'a str>,
    headers: Vec<(&'a str, &'a str)>,
    body: &'a [u8],
}

impl<'a> Part<'a> {
    fn text(name: &'a str, body: &'a [u8]) -> Self {
        Self {
            name,
            file_name: None,
            content_type: None,
            headers: Vec::new(),
            body,
        }
    }

    fn file(name: &'a str, file_name: &'a str, content_type: &'a str, body: &'a [u8]) -> Self {
        Self {
            name,
            file_name: Some(file_name),
            content_type: Some(content_type),
            headers: Vec::new(),
            body,
        }
    }

    fn with_header(mut self, name: &'a str, value: &'a str) -> Self {
        self.headers.push((name, value));
        self
    }
}

fn multipart_body(boundary: &str, parts: &[Part<'_>]) -> Bytes {
    let mut body = Vec::new();
    for part in parts {
        body.extend_from_slice(format!("--{boundary}\r\n").as_bytes());
        body.extend_from_slice(
            format!("Content-Disposition: form-data; name=\"{}\"", part.name).as_bytes(),
        );
        if let Some(file_name) = part.file_name {
            body.extend_from_slice(format!("; filename=\"{file_name}\"").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        if let Some(content_type) = part.content_type {
            body.extend_from_slice(format!("Content-Type: {content_type}\r\n").as_bytes());
        }
        for (name, value) in &part.headers {
            body.extend_from_slice(format!("{name}: {value}\r\n").as_bytes());
        }
        body.extend_from_slice(b"\r\n");
        body.extend_from_slice(part.body);
        body.extend_from_slice(b"\r\n");
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    Bytes::from(body)
}

struct ReadField {
    name: String,
    headers: HeaderMap,
    body: Bytes,
}

async fn read_fields(body: Bytes, content_type: &str) -> Vec<ReadField> {
    let boundary = multer::parse_boundary(content_type).expect("encoded boundary");
    let input = stream::once(async move { Ok::<Bytes, Infallible>(body) });
    let mut multipart = Multipart::new(input, boundary);
    let mut fields = Vec::new();
    while let Some(field) = multipart.next_field().await.expect("encoded field") {
        let name = field.name().expect("field name").to_owned();
        let headers = field.headers().clone();
        let body = field.bytes().await.expect("field body");
        fields.push(ReadField {
            name,
            headers,
            body,
        });
    }
    fields
}
