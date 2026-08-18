use std::{borrow::Cow, collections::BTreeMap, fmt, ops::Range};

use any2api_domain::ProtocolOperation;
use any2api_payload_buffer::{PayloadBuffer, PayloadBufferError};
use bytes::Bytes;
use serde::{
    Deserialize, Deserializer,
    de::{DeserializeOwned, DeserializeSeed, IgnoredAny, MapAccess, Visitor},
};
use serde_json::{Value, value::RawValue};

use crate::ProtocolError;

/// A validated top-level JSON object backed by one shared wire allocation.
#[derive(Clone)]
pub struct RawJsonPayload {
    body: Bytes,
    fields: BTreeMap<String, Range<usize>>,
    has_duplicate_fields: bool,
}

impl RawJsonPayload {
    pub(crate) fn parse(body: Bytes) -> Result<Self, ProtocolError> {
        let first = body
            .iter()
            .copied()
            .find(|byte| !byte.is_ascii_whitespace());
        if first != Some(b'{') {
            serde_json::from_slice::<serde::de::IgnoredAny>(&body).map_err(|_| invalid_json())?;
            return Err(ProtocolError::InvalidPayload(
                "request body must be a JSON object".into(),
            ));
        }
        let borrowed = borrowed_object_entries(&body).map_err(|_| invalid_json())?;
        let base = body.as_ptr() as usize;
        let end = base.saturating_add(body.len());
        let mut fields = BTreeMap::new();
        let mut has_duplicate_fields = false;
        for (name, value) in borrowed {
            let start = value.get().as_ptr() as usize;
            let value_end = start.saturating_add(value.get().len());
            if start < base || value_end > end {
                return Err(invalid_json());
            }
            has_duplicate_fields |= fields
                .insert(name, (start - base)..(value_end - base))
                .is_some();
        }
        Ok(Self {
            body,
            fields,
            has_duplicate_fields,
        })
    }

    pub(crate) fn field(&self, name: &str) -> Option<&[u8]> {
        self.fields.get(name).map(|range| &self.body[range.clone()])
    }

    pub(crate) fn parse_field<T: DeserializeOwned>(
        &self,
        name: &str,
    ) -> Option<Result<T, serde_json::Error>> {
        self.field(name).map(serde_json::from_slice)
    }

    pub(crate) fn encode(
        &self,
        operation: ProtocolOperation,
        upstream_model: &str,
    ) -> Result<Bytes, ProtocolError> {
        let model_matches = self
            .parse_field::<String>("model")
            .transpose()
            .map_err(|_| ProtocolError::InvalidPayload("model must be a non-empty string".into()))?
            .is_some_and(|model| model == upstream_model);
        let removes_stream = !operation.allows_stream() && self.fields.contains_key("stream");
        if model_matches && !removes_stream && !self.has_duplicate_fields {
            return Ok(self.body.clone());
        }

        let mut encoded = output_buffer(self.body.len())?;
        encoded.extend_from_slice(b"{").map_err(buffer_error)?;
        let mut first = true;
        for (name, range) in &self.fields {
            if name == "stream" && !operation.allows_stream() {
                continue;
            }
            write_field_prefix(&mut encoded, &mut first, name)?;
            if name == "model" {
                serde_json::to_writer(&mut encoded, upstream_model).map_err(json_encode_error)?;
            } else {
                encoded
                    .extend_from_slice(&self.body[range.clone()])
                    .map_err(buffer_error)?;
            }
        }
        encoded.extend_from_slice(b"}").map_err(buffer_error)?;
        Ok(encoded.freeze().into_bytes())
    }

    pub(crate) fn replace_raw_field(
        &mut self,
        field: &str,
        replacement: &[u8],
    ) -> Result<(), ProtocolError> {
        if !self.fields.contains_key(field) {
            return Ok(());
        }
        let mut encoded = output_buffer(self.body.len().max(replacement.len()))?;
        encoded.extend_from_slice(b"{").map_err(buffer_error)?;
        let mut first = true;
        let mut fields = BTreeMap::new();
        for (name, range) in &self.fields {
            write_field_prefix(&mut encoded, &mut first, name)?;
            let start = encoded.len();
            if name == field {
                encoded
                    .extend_from_slice(replacement)
                    .map_err(buffer_error)?;
            } else {
                encoded
                    .extend_from_slice(&self.body[range.clone()])
                    .map_err(buffer_error)?;
            }
            fields.insert(name.clone(), start..encoded.len());
        }
        encoded.extend_from_slice(b"}").map_err(buffer_error)?;
        self.body = encoded.freeze().into_bytes();
        self.fields = fields;
        self.has_duplicate_fields = false;
        Ok(())
    }

    pub(crate) fn to_value(&self) -> Result<Value, ProtocolError> {
        serde_json::from_slice(&self.body).map_err(|_| invalid_json())
    }
}

impl fmt::Debug for RawJsonPayload {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RawJson")
            .field("body_bytes", &self.body.len())
            .field("field_count", &self.fields.len())
            .field("has_duplicate_fields", &self.has_duplicate_fields)
            .finish()
    }
}

pub(crate) fn borrowed_object(bytes: &[u8]) -> serde_json::Result<BTreeMap<String, &RawValue>> {
    serde_json::from_slice(bytes)
}

fn borrowed_object_entries(bytes: &[u8]) -> serde_json::Result<Vec<(String, &RawValue)>> {
    serde_json::from_slice(bytes).map(|entries: RawObjectEntries<'_>| entries.0)
}

struct RawObjectEntries<'a>(Vec<(String, &'a RawValue)>);

impl<'de> Deserialize<'de> for RawObjectEntries<'de> {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_map(RawObjectEntriesVisitor)
    }
}

struct RawObjectEntriesVisitor;

impl<'de> Visitor<'de> for RawObjectEntriesVisitor {
    type Value = RawObjectEntries<'de>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        let mut entries = Vec::with_capacity(map.size_hint().unwrap_or(0));
        while let Some((name, value)) = map.next_entry::<String, &'de RawValue>()? {
            entries.push((name, value));
        }
        Ok(RawObjectEntries(entries))
    }
}

pub(crate) fn raw_array(bytes: &[u8]) -> Option<Vec<&RawValue>> {
    serde_json::from_slice(bytes).ok()
}

pub(crate) fn raw_string(bytes: &[u8]) -> Option<String> {
    serde_json::from_slice(bytes).ok()
}

pub(crate) fn object_field<'a>(bytes: &'a [u8], name: &str) -> Option<&'a [u8]> {
    object_field_raw(bytes, name).map(|value| value.get().as_bytes())
}

/// Extract one top-level field of a JSON object without materializing any of
/// its siblings; when the key repeats, the last occurrence wins.
pub(crate) fn object_field_raw<'a>(bytes: &'a [u8], name: &str) -> Option<&'a RawValue> {
    let [value] = top_fields(bytes, [name]);
    value
}

/// Scan the top-level fields of a JSON object once, keeping the last
/// occurrence of every requested name; all-`None` when the input is not a
/// valid JSON object.
pub(crate) fn top_fields<'de, const N: usize>(
    json: &'de [u8],
    names: [&str; N],
) -> [Option<&'de RawValue>; N] {
    let mut fields = [None; N];
    scan_object_fields(json, &names, &mut |index, value| {
        fields[index] = Some(value)
    });
    fields
}

/// Walk the top-level fields of a JSON object, invoking `on_match` with the
/// index into `names` for every occurrence of a requested name. Unmatched
/// values are skipped without allocating. Returns false for non-objects.
pub(crate) fn scan_object_fields<'de>(
    json: &'de [u8],
    names: &[&str],
    on_match: &mut dyn FnMut(usize, &'de RawValue),
) -> bool {
    let mut deserializer = serde_json::Deserializer::from_slice(json);
    if deserializer
        .deserialize_map(ObjectScanVisitor { names, on_match })
        .is_err()
    {
        return false;
    }
    deserializer.end().is_ok()
}

struct ObjectScanVisitor<'s, 'de> {
    names: &'s [&'s str],
    on_match: &'s mut dyn FnMut(usize, &'de RawValue),
}

impl<'s, 'de> Visitor<'de> for ObjectScanVisitor<'s, 'de> {
    type Value = ();

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object")
    }

    fn visit_map<A>(self, mut map: A) -> Result<Self::Value, A::Error>
    where
        A: MapAccess<'de>,
    {
        while let Some(index) = map.next_key_seed(KeyIndex { names: self.names })? {
            if let Some(index) = index {
                (self.on_match)(index, map.next_value::<&'de RawValue>()?);
            } else {
                map.next_value::<IgnoredAny>()?;
            }
        }
        Ok(())
    }
}

struct KeyIndex<'s> {
    names: &'s [&'s str],
}

impl<'de> DeserializeSeed<'de> for KeyIndex<'_> {
    type Value = Option<usize>;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: Deserializer<'de>,
    {
        deserializer.deserialize_str(self)
    }
}

impl Visitor<'_> for KeyIndex<'_> {
    type Value = Option<usize>;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON object key")
    }

    fn visit_str<E: serde::de::Error>(self, key: &str) -> Result<Self::Value, E> {
        Ok(self.names.iter().position(|name| *name == key))
    }
}

/// Decode a raw JSON string, borrowing the input unless it contains escapes;
/// `None` when the value is not a string.
pub(crate) fn json_string(raw: &RawValue) -> Option<Cow<'_, str>> {
    let text = raw.get();
    if !text.starts_with('"') {
        return None;
    }
    serde_json::from_str::<&str>(text)
        .map(Cow::Borrowed)
        .or_else(|_| serde_json::from_str::<String>(text).map(Cow::Owned))
        .ok()
}

/// Offset range of `inner` within `outer` when `inner` borrows from the same
/// allocation, established without any unsafe pointer dereference.
pub(crate) fn subslice_range(outer: &[u8], inner: &[u8]) -> Option<Range<usize>> {
    let offset = (inner.as_ptr() as usize).checked_sub(outer.as_ptr() as usize)?;
    let end = offset.checked_add(inner.len())?;
    (end <= outer.len()).then_some(offset..end)
}

/// Rebuild `source` with each non-overlapping, ascending span replaced by
/// `replacement`; every other byte is forwarded verbatim.
pub(crate) fn splice_ranges(
    source: &[u8],
    spans: &[Range<usize>],
    replacement: &[u8],
) -> Result<Bytes, ProtocolError> {
    let removed_bytes = spans.iter().try_fold(0_usize, |total, span| {
        total.checked_add(span.len()).ok_or_else(encode_error)
    })?;
    let replacement_bytes = spans
        .len()
        .checked_mul(replacement.len())
        .ok_or_else(encode_error)?;
    let output_len = source
        .len()
        .checked_sub(removed_bytes)
        .and_then(|length| length.checked_add(replacement_bytes))
        .ok_or_else(encode_error)?;
    let mut output =
        PayloadBuffer::with_capacity_hint(Some(output_len), output_len).map_err(buffer_error)?;
    let mut cursor = 0;
    for span in spans {
        output
            .extend_from_slice(&source[cursor..span.start])
            .map_err(buffer_error)?;
        output
            .extend_from_slice(replacement)
            .map_err(buffer_error)?;
        cursor = span.end;
    }
    output
        .extend_from_slice(&source[cursor..])
        .map_err(buffer_error)?;
    Ok(output.freeze().into_bytes())
}

fn write_field_prefix(
    encoded: &mut PayloadBuffer,
    first: &mut bool,
    name: &str,
) -> Result<(), ProtocolError> {
    if !*first {
        encoded.extend_from_slice(b",").map_err(buffer_error)?;
    }
    *first = false;
    serde_json::to_writer(&mut *encoded, name).map_err(json_encode_error)?;
    encoded.extend_from_slice(b":").map_err(buffer_error)?;
    Ok(())
}

fn output_buffer(expected_len: usize) -> Result<PayloadBuffer, ProtocolError> {
    PayloadBuffer::with_capacity_hint(Some(expected_len), usize::MAX).map_err(buffer_error)
}

fn buffer_error(_error: PayloadBufferError) -> ProtocolError {
    ProtocolError::Internal("protocol payload allocation failed".into())
}

fn json_encode_error(error: serde_json::Error) -> ProtocolError {
    if error.is_io() {
        buffer_error(PayloadBufferError::AllocationFailed)
    } else {
        encode_error()
    }
}

fn invalid_json() -> ProtocolError {
    ProtocolError::InvalidPayload("request body must be valid JSON".into())
}

fn encode_error() -> ProtocolError {
    ProtocolError::InvalidPayload("request JSON could not be encoded".into())
}

#[cfg(test)]
mod tests {
    use any2api_domain::ProtocolOperation;
    use bytes::Bytes;

    use super::RawJsonPayload;

    #[test]
    fn unchanged_request_reuses_wire_allocation_and_rewrites_only_when_needed() {
        let body = Bytes::from_static(
            br#" { "model" : "gpt", "stream": true, "unknown": {"nested":1} } "#,
        );
        let payload = RawJsonPayload::parse(body.clone()).expect("raw JSON");
        let reused = payload
            .encode(ProtocolOperation::Responses, "gpt")
            .expect("reused request");
        assert_eq!(reused.as_ptr(), body.as_ptr());

        let rewritten = payload
            .encode(ProtocolOperation::ResponsesCompact, "upstream")
            .expect("rewritten request");
        let value: serde_json::Value = serde_json::from_slice(&rewritten).expect("JSON");
        assert_eq!(value["model"], "upstream");
        assert!(value.get("stream").is_none());
        assert_eq!(value["unknown"]["nested"], 1);
    }

    #[test]
    fn rejects_invalid_json_and_non_object_roots_without_building_a_value_tree() {
        assert!(RawJsonPayload::parse(Bytes::from_static(b"{")).is_err());
        let error = RawJsonPayload::parse(Bytes::from_static(b"[]")).expect_err("object root");
        assert!(error.to_string().contains("JSON object"));
    }

    #[test]
    fn duplicate_top_level_fields_are_collapsed_before_forwarding() {
        let body = Bytes::from_static(br#"{"model":"wrong","model":"gpt","input":[]}"#);
        let payload = RawJsonPayload::parse(body.clone()).expect("raw JSON");
        let encoded = payload
            .encode(ProtocolOperation::Responses, "gpt")
            .expect("normalized request");
        assert_ne!(encoded.as_ptr(), body.as_ptr());
        assert_eq!(
            encoded,
            Bytes::from_static(br#"{"input":[],"model":"gpt"}"#)
        );
    }

    #[test]
    fn model_and_stream_materialization_have_explicit_wire_goldens() {
        let model = RawJsonPayload::parse(Bytes::from_static(
            br#" { "z" : {"opaque":true}, "model" : "public", "a": [1, 2] } "#,
        ))
        .expect("raw JSON");
        assert_eq!(
            model
                .encode(ProtocolOperation::Responses, "upstream")
                .expect("model rewrite"),
            Bytes::from_static(br#"{"a":[1, 2],"model":"upstream","z":{"opaque":true}}"#)
        );

        let stream = RawJsonPayload::parse(Bytes::from_static(
            br#"{"z":0,"stream" : true,"model":"gpt","input":"hello"}"#,
        ))
        .expect("raw JSON");
        assert_eq!(
            stream
                .encode(ProtocolOperation::ResponsesCompact, "gpt")
                .expect("stream removal"),
            Bytes::from_static(br#"{"input":"hello","model":"gpt","z":0}"#)
        );
    }

    #[test]
    fn large_raw_rewrite_preserves_opaque_content() {
        let content = "x".repeat(300 * 1024);
        let body = Bytes::from(format!(
            "{{\"model\":\"public\",\"input\":\"{content}\",\"stream\":true}}"
        ));
        let payload = RawJsonPayload::parse(body).expect("raw JSON");

        let encoded = payload
            .encode(ProtocolOperation::ResponsesCompact, "upstream")
            .expect("large rewrite");
        let value: serde_json::Value = serde_json::from_slice(&encoded).expect("rewritten JSON");

        assert_eq!(value["model"], "upstream");
        assert_eq!(value["input"].as_str(), Some(content.as_str()));
        assert!(value.get("stream").is_none());
    }
}
