use std::convert::Infallible;

use any2api_domain::ProtocolOperation;
use bytes::Bytes;
use futures_util::stream;
use http::{HeaderMap, HeaderValue, Method, Uri, header};
use multer::Multipart;

use crate::api::IngressRequest;

pub(super) fn multipart_request(boundary: &str, body: Bytes) -> IngressRequest {
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
pub(super) struct Part<'a> {
    name: &'a str,
    file_name: Option<&'a str>,
    content_type: Option<&'a str>,
    headers: Vec<(&'a str, &'a str)>,
    body: &'a [u8],
}

impl<'a> Part<'a> {
    pub(super) fn text(name: &'a str, body: &'a [u8]) -> Self {
        Self {
            name,
            file_name: None,
            content_type: None,
            headers: Vec::new(),
            body,
        }
    }

    pub(super) fn file(
        name: &'a str,
        file_name: &'a str,
        content_type: &'a str,
        body: &'a [u8],
    ) -> Self {
        Self {
            name,
            file_name: Some(file_name),
            content_type: Some(content_type),
            headers: Vec::new(),
            body,
        }
    }

    pub(super) fn with_header(mut self, name: &'a str, value: &'a str) -> Self {
        self.headers.push((name, value));
        self
    }
}

pub(super) fn multipart_body(boundary: &str, parts: &[Part<'_>]) -> Bytes {
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

pub(super) struct ReadField {
    pub(super) name: String,
    pub(super) headers: HeaderMap,
    pub(super) body: Bytes,
}

pub(super) async fn read_fields(body: Bytes, content_type: &str) -> Vec<ReadField> {
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
