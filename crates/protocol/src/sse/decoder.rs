use any2api_payload_buffer::{PayloadBuffer, PayloadBufferError};

use crate::{ProtocolError, api::SseFrame};

pub struct SseDecoder {
    buffer: PayloadBuffer,
    max_frame_bytes: usize,
    frame_ready: bool,
}

impl SseDecoder {
    #[must_use]
    pub fn new(max_frame_bytes: usize) -> Self {
        let max_frame_bytes = max_frame_bytes.max(1);
        Self {
            buffer: PayloadBuffer::new(max_frame_bytes.saturating_add(1)),
            max_frame_bytes,
            frame_ready: false,
        }
    }

    /// Consumes at most one frame prefix. A zero-byte consume means a trailing
    /// bare CR became a complete delimiter after inspecting the next byte.
    pub fn push(&mut self, chunk: &[u8]) -> Result<usize, ProtocolError> {
        if chunk.is_empty() || self.frame_ready {
            return Ok(0);
        }
        let input = &chunk[..self.next_input_limit().min(chunk.len())];
        let event_end = event_end_after_append(self.buffer.as_slice(), input);
        let consumed = event_end.unwrap_or(input.len());
        self.buffer
            .extend_from_slice(&input[..consumed])
            .map_err(buffer_error)?;
        self.frame_ready = event_end.is_some();
        Ok(consumed)
    }

    pub fn next_input_limit(&self) -> usize {
        if self.frame_ready {
            return 0;
        }
        self.max_frame_bytes
            .saturating_sub(self.buffer.len())
            .saturating_add(1)
    }

    pub fn next_frame(&mut self) -> Result<Option<SseFrame>, ProtocolError> {
        if !self.frame_ready {
            self.check_buffer_limit()?;
            return Ok(None);
        }
        self.check_buffer_limit()?;
        let buffer = std::mem::replace(
            &mut self.buffer,
            PayloadBuffer::new(self.max_frame_bytes.saturating_add(1)),
        );
        self.frame_ready = false;
        Ok(Some(SseFrame(buffer.freeze().into_bytes())))
    }

    pub fn finish(&mut self) -> Result<Option<SseFrame>, ProtocolError> {
        if self.buffer.is_empty() {
            return Ok(None);
        }
        self.check_buffer_limit()?;
        let buffer = std::mem::replace(
            &mut self.buffer,
            PayloadBuffer::new(self.max_frame_bytes.saturating_add(1)),
        );
        self.frame_ready = false;
        Ok(Some(SseFrame(buffer.freeze().into_bytes())))
    }

    fn check_buffer_limit(&self) -> Result<(), ProtocolError> {
        if self.buffer.len() > self.max_frame_bytes {
            Err(frame_limit_error())
        } else {
            Ok(())
        }
    }
}

fn buffer_error(_error: PayloadBufferError) -> ProtocolError {
    ProtocolError::Internal("SSE frame allocation failed".into())
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

fn event_end_after_append(buffer: &[u8], input: &[u8]) -> Option<usize> {
    const BOUNDARY_BYTES: usize = 3;
    const PREFIX_BYTES: usize = 4;

    let tail = &buffer[buffer.len().saturating_sub(BOUNDARY_BYTES)..];
    let prefix = &input[..input.len().min(PREFIX_BYTES)];
    let mut boundary = [0_u8; BOUNDARY_BYTES + PREFIX_BYTES];
    boundary[..tail.len()].copy_from_slice(tail);
    boundary[tail.len()..tail.len() + prefix.len()].copy_from_slice(prefix);
    let crossing = find_event_end(&boundary[..tail.len() + prefix.len()], 0)
        .filter(|end| *end >= tail.len())
        .map(|end| end - tail.len());
    let within_input = find_event_end(input, 0);
    match (crossing, within_input) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(end), None) | (None, Some(end)) => Some(end),
        (None, None) => None,
    }
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
