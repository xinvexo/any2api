use bytes::BytesMut;

use crate::{ProtocolError, api::SseFrame};

#[derive(Debug)]
pub struct SseDecoder {
    buffer: BytesMut,
    max_frame_bytes: usize,
    scan_from: usize,
}

impl SseDecoder {
    #[must_use]
    pub fn new(max_frame_bytes: usize) -> Self {
        Self {
            buffer: BytesMut::new(),
            max_frame_bytes: max_frame_bytes.max(1),
            scan_from: 0,
        }
    }

    pub fn push(&mut self, chunk: &[u8]) {
        self.buffer.extend_from_slice(chunk);
    }

    pub fn next_input_limit(&self) -> usize {
        self.max_frame_bytes
            .saturating_sub(self.buffer.len())
            .saturating_add(1)
    }

    pub fn next_frame(&mut self) -> Result<Option<SseFrame>, ProtocolError> {
        let Some(end) = find_event_end(&self.buffer, self.scan_from) else {
            self.scan_from = self.buffer.len().saturating_sub(3);
            self.check_buffer_limit()?;
            return Ok(None);
        };
        if end > self.max_frame_bytes {
            return Err(frame_limit_error());
        }
        self.scan_from = 0;
        Ok(Some(SseFrame(self.buffer.split_to(end).freeze())))
    }

    pub fn finish(&mut self) -> Result<Option<SseFrame>, ProtocolError> {
        if self.buffer.is_empty() {
            return Ok(None);
        }
        self.check_buffer_limit()?;
        Ok(Some(SseFrame(self.buffer.split().freeze())))
    }

    fn check_buffer_limit(&self) -> Result<(), ProtocolError> {
        if self.buffer.len() > self.max_frame_bytes {
            Err(frame_limit_error())
        } else {
            Ok(())
        }
    }
}

fn frame_limit_error() -> ProtocolError {
    ProtocolError::InvalidPayload("SSE frame exceeded the configured limit".into())
}

fn find_event_end(bytes: &[u8], start: usize) -> Option<usize> {
    for index in start.min(bytes.len())..bytes.len() {
        let Some(first) = line_ending_len(bytes, index) else {
            continue;
        };
        let second_at = index + first;
        if let Some(second) = line_ending_len(bytes, second_at) {
            return Some(second_at + second);
        }
    }
    None
}

fn line_ending_len(bytes: &[u8], index: usize) -> Option<usize> {
    match *bytes.get(index)? {
        b'\n' => Some(1),
        b'\r' => match bytes.get(index + 1) {
            Some(b'\n') => Some(2),
            Some(_) => Some(1),
            None => None,
        },
        _ => None,
    }
}
