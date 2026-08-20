#![allow(
    unsafe_code,
    reason = "this module is the audited zstd streaming decoder FFI boundary"
)]

use std::{ffi::c_void, ptr::NonNull};

use any2api_payload_buffer::{PayloadBuffer, PayloadBufferError};
use bytes::Bytes;
use thiserror::Error;

use crate::allocator::AllocatorState;

const DECODE_CHUNK_BYTES: usize = 16 * 1024;

pub struct Decoder {
    allocator: Box<AllocatorState>,
    context: NonNull<zstd_sys::ZSTD_DStream>,
}

impl Decoder {
    pub fn try_new() -> Option<Self> {
        let mut allocator = Box::new(AllocatorState::new());
        // SAFETY: the custom allocator's boxed state has a stable address and
        // remains alive until after `ZSTD_freeDStream` completes in `Drop`.
        let context = unsafe { zstd_sys::ZSTD_createDStream_advanced(allocator.custom_memory()) };
        let context = NonNull::new(context)?;
        Some(Self { allocator, context })
    }

    pub fn decode(&mut self, input: &[u8], max_output_bytes: usize) -> Result<Bytes, DecodeError> {
        self.allocator.begin_operation();
        self.reset()?;
        let mut output = PayloadBuffer::with_capacity_hint(Some(input.len()), max_output_bytes)
            .map_err(map_buffer_error)?;
        let mut source = zstd_sys::ZSTD_inBuffer {
            src: input.as_ptr().cast::<c_void>(),
            size: input.len(),
            pos: 0,
        };
        let mut frame_finished = false;

        loop {
            if frame_finished {
                if source.pos == source.size {
                    break;
                }
                self.reset()?;
                frame_finished = false;
            }

            let remaining = max_output_bytes
                .saturating_add(1)
                .saturating_sub(output.len());
            let mut chunk = [0_u8; DECODE_CHUNK_BYTES];
            let mut destination = zstd_sys::ZSTD_outBuffer {
                dst: chunk.as_mut_ptr().cast::<c_void>(),
                size: remaining.min(DECODE_CHUNK_BYTES),
                pos: 0,
            };
            let source_position = source.pos;
            // SAFETY: context, source and destination remain valid for this
            // call; zstd is bounded by the declared input/output lengths.
            let result = unsafe {
                zstd_sys::ZSTD_decompressStream(
                    self.context.as_ptr(),
                    &mut destination,
                    &mut source,
                )
            };
            if is_zstd_error(result) {
                return Err(self.error_for_zstd_failure());
            }

            let produced = destination.pos;
            if output.len().saturating_add(produced) > max_output_bytes {
                return Err(DecodeError::TooLarge);
            }
            output
                .extend_from_slice(&chunk[..produced])
                .map_err(map_buffer_error)?;

            if result == 0 {
                frame_finished = true;
                continue;
            }
            if produced == 0 && source.pos == source_position {
                return Err(DecodeError::Invalid);
            }
        }

        Ok(output.freeze().into_bytes())
    }

    fn reset(&mut self) -> Result<(), DecodeError> {
        // SAFETY: `context` is a live zstd stream exclusively borrowed here.
        let result = unsafe { zstd_sys::ZSTD_initDStream(self.context.as_ptr()) };
        if is_zstd_error(result) {
            return Err(self.error_for_zstd_failure());
        }
        Ok(())
    }

    fn error_for_zstd_failure(&self) -> DecodeError {
        if self.allocator.allocation_failed() {
            DecodeError::AllocationFailed
        } else {
            DecodeError::Invalid
        }
    }
}

impl Drop for Decoder {
    fn drop(&mut self) {
        // SAFETY: `context` was created by zstd and is freed exactly once,
        // before the custom allocator state is dropped.
        let _ = unsafe { zstd_sys::ZSTD_freeDStream(self.context.as_ptr()) };
    }
}

fn is_zstd_error(result: usize) -> bool {
    // SAFETY: this pure zstd predicate accepts every size_t result.
    unsafe { zstd_sys::ZSTD_isError(result) != 0 }
}

fn map_buffer_error(error: PayloadBufferError) -> DecodeError {
    match error {
        PayloadBufferError::TooLarge => DecodeError::TooLarge,
        PayloadBufferError::AllocationFailed => DecodeError::AllocationFailed,
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum DecodeError {
    #[error("zstd input is invalid")]
    Invalid,
    #[error("zstd output exceeded its bounded size")]
    TooLarge,
    #[error("zstd workspace allocation failed")]
    AllocationFailed,
}

#[cfg(test)]
mod tests {
    use std::io::Write;

    use super::*;

    #[test]
    fn decoder_supports_normal_and_concatenated_frames() {
        let first = zstd::stream::encode_all(&b"first"[..], 3).expect("first frame");
        let second = zstd::stream::encode_all(&b"second"[..], 3).expect("second frame");
        let mut input = first;
        input.extend_from_slice(&second);
        let mut decoder = Decoder::try_new().expect("decoder");
        assert_eq!(
            decoder.decode(&input, 128).expect("decode frames"),
            Bytes::from_static(b"firstsecond")
        );
    }

    #[test]
    fn decoder_rejects_invalid_and_oversized_output() {
        let mut decoder = Decoder::try_new().expect("decoder");
        assert_eq!(decoder.decode(b"invalid", 128), Err(DecodeError::Invalid));
        let input = zstd::stream::encode_all(&vec![b'x'; 1024][..], 1).expect("large frame");
        assert_eq!(decoder.decode(&input, 128), Err(DecodeError::TooLarge));
    }

    #[test]
    fn streaming_two_mebibyte_window_keeps_its_large_workspace_mapped() {
        let payload = (0..1_500_000)
            .map(|index| (index as u8).wrapping_mul(31))
            .collect::<Vec<_>>();
        let mut encoder = zstd::stream::Encoder::new(Vec::new(), 1).expect("create encoder");
        encoder.window_log(21).expect("set 2 MiB window");
        encoder
            .include_contentsize(false)
            .expect("omit content size");
        encoder.write_all(&payload).expect("encode payload");
        let input = encoder.finish().expect("finish frame");
        let mut decoder = Decoder::try_new().expect("decoder");

        assert_eq!(
            decoder
                .decode(&input, payload.len())
                .expect("decode payload"),
            payload
        );
        assert!(decoder.allocator.has_mapped_allocation());
    }
}
