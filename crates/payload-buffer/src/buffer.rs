use std::io::{self, Write};

use bytes::Bytes;
use memmap2::MmapMut;
use thiserror::Error;

const MAPPED_ALLOCATION_THRESHOLD_BYTES: usize = 256 * 1024;

pub struct PayloadBuffer {
    storage: Storage,
    len: usize,
    max_len: usize,
}

enum Storage {
    Heap(Vec<u8>),
    Mapped(MmapMut),
}

impl PayloadBuffer {
    #[must_use]
    pub const fn new(max_len: usize) -> Self {
        Self {
            storage: Storage::Heap(Vec::new()),
            len: 0,
            max_len,
        }
    }

    /// Uses a bounded length hint only to choose initial storage. A hint above
    /// the actual object limit is ignored; every appended byte is still checked.
    pub fn with_capacity_hint(
        expected_len: Option<usize>,
        max_len: usize,
    ) -> Result<Self, PayloadBufferError> {
        let capacity = expected_len
            .filter(|expected| *expected <= max_len)
            .unwrap_or_default();
        let storage = allocate_storage(capacity)?;
        Ok(Self {
            storage,
            len: 0,
            max_len,
        })
    }

    pub fn extend_from_slice(&mut self, bytes: &[u8]) -> Result<(), PayloadBufferError> {
        let next_len = self
            .len
            .checked_add(bytes.len())
            .filter(|next_len| *next_len <= self.max_len)
            .ok_or(PayloadBufferError::TooLarge)?;
        self.ensure_capacity(next_len)?;
        match &mut self.storage {
            Storage::Heap(storage) => storage.extend_from_slice(bytes),
            Storage::Mapped(storage) => storage[self.len..next_len].copy_from_slice(bytes),
        }
        self.len = next_len;
        Ok(())
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub fn as_slice(&self) -> &[u8] {
        match &self.storage {
            Storage::Heap(storage) => storage.as_slice(),
            Storage::Mapped(storage) => &storage[..self.len],
        }
    }

    #[must_use]
    pub fn freeze(self) -> FrozenPayload {
        let (bytes, allocated_bytes) = match self.storage {
            Storage::Heap(storage) => {
                let allocated_bytes = storage.capacity();
                (Bytes::from(storage), allocated_bytes)
            }
            Storage::Mapped(storage) => {
                let allocated_bytes = storage.len();
                let bytes = Bytes::from_owner(storage).slice(..self.len);
                (bytes, allocated_bytes)
            }
        };
        FrozenPayload {
            bytes,
            allocated_bytes,
        }
    }

    fn ensure_capacity(&mut self, required: usize) -> Result<(), PayloadBufferError> {
        let current = match &self.storage {
            Storage::Heap(storage) => storage.capacity(),
            Storage::Mapped(storage) => storage.len(),
        };
        if required < MAPPED_ALLOCATION_THRESHOLD_BYTES {
            if required <= current {
                return Ok(());
            }
            let Storage::Heap(storage) = &mut self.storage else {
                unreachable!("mapped payload capacity cannot be below its threshold")
            };
            storage
                .try_reserve(required.saturating_sub(storage.len()))
                .map_err(|_| PayloadBufferError::AllocationFailed)?;
            return Ok(());
        }
        if matches!(self.storage, Storage::Mapped(_)) && required <= current {
            return Ok(());
        }

        let capacity = required.max(current.saturating_mul(2)).min(self.max_len);
        let mut replacement = map_anon(capacity)?;
        match &self.storage {
            Storage::Heap(storage) => replacement[..self.len].copy_from_slice(storage),
            Storage::Mapped(storage) => {
                replacement[..self.len].copy_from_slice(&storage[..self.len]);
            }
        }
        self.storage = Storage::Mapped(replacement);
        Ok(())
    }
}

impl Write for PayloadBuffer {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        self.extend_from_slice(bytes).map_err(buffer_io_error)?;
        Ok(bytes.len())
    }

    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

fn buffer_io_error(error: PayloadBufferError) -> io::Error {
    let kind = match error {
        PayloadBufferError::TooLarge => io::ErrorKind::WriteZero,
        PayloadBufferError::AllocationFailed => io::ErrorKind::OutOfMemory,
    };
    io::Error::new(kind, error)
}

fn allocate_storage(capacity: usize) -> Result<Storage, PayloadBufferError> {
    if capacity >= MAPPED_ALLOCATION_THRESHOLD_BYTES {
        return map_anon(capacity).map(Storage::Mapped);
    }
    let mut storage = Vec::new();
    storage
        .try_reserve_exact(capacity)
        .map_err(|_| PayloadBufferError::AllocationFailed)?;
    Ok(Storage::Heap(storage))
}

fn map_anon(capacity: usize) -> Result<MmapMut, PayloadBufferError> {
    MmapMut::map_anon(capacity).map_err(|_| PayloadBufferError::AllocationFailed)
}

pub struct FrozenPayload {
    bytes: Bytes,
    allocated_bytes: usize,
}

impl FrozenPayload {
    #[must_use]
    pub const fn allocated_bytes(&self) -> usize {
        self.allocated_bytes
    }

    #[must_use]
    pub fn into_bytes(self) -> Bytes {
        self.bytes
    }

    #[must_use]
    pub fn into_parts(self) -> (Bytes, usize) {
        (self.bytes, self.allocated_bytes)
    }
}

impl AsRef<[u8]> for FrozenPayload {
    fn as_ref(&self) -> &[u8] {
        self.bytes.as_ref()
    }
}

#[derive(Debug, Error, Eq, PartialEq)]
pub enum PayloadBufferError {
    #[error("payload exceeded its bounded size")]
    TooLarge,
    #[error("payload allocation failed")]
    AllocationFailed,
}

#[cfg(test)]
mod tests {
    use std::io::{ErrorKind, Write};

    use super::*;

    #[test]
    fn small_payload_stays_on_the_heap_and_reports_capacity() {
        let mut buffer = PayloadBuffer::with_capacity_hint(Some(1024), 2048).expect("buffer");
        assert!(matches!(buffer.storage, Storage::Heap(_)));
        buffer.extend_from_slice(b"small").expect("content");

        let frozen = buffer.freeze();
        assert_eq!(frozen.as_ref(), b"small");
        assert_eq!(frozen.allocated_bytes(), 1024);
    }

    #[test]
    fn large_payload_uses_mapped_storage_with_exact_owned_bytes() {
        let size = MAPPED_ALLOCATION_THRESHOLD_BYTES;
        let mut buffer = PayloadBuffer::with_capacity_hint(Some(size), size).expect("buffer");
        assert!(matches!(buffer.storage, Storage::Mapped(_)));
        buffer.extend_from_slice(b"first chunk").expect("prefix");
        buffer
            .extend_from_slice(&vec![b'x'; size - 11])
            .expect("remainder");

        let frozen = buffer.freeze();
        assert_eq!(&frozen.as_ref()[..11], b"first chunk");
        assert_eq!(frozen.as_ref().len(), size);
        assert_eq!(frozen.allocated_bytes(), size);
    }

    #[test]
    fn growing_payload_moves_from_heap_to_mapped_storage() {
        let mut buffer = PayloadBuffer::new(MAPPED_ALLOCATION_THRESHOLD_BYTES);
        buffer
            .extend_from_slice(&vec![b'x'; MAPPED_ALLOCATION_THRESHOLD_BYTES - 1])
            .expect("heap content");
        assert!(matches!(buffer.storage, Storage::Heap(_)));
        buffer.extend_from_slice(b"y").expect("mapped content");
        assert!(matches!(buffer.storage, Storage::Mapped(_)));
        assert_eq!(buffer.freeze().as_ref().last(), Some(&b'y'));
    }

    #[test]
    fn oversized_hint_is_ignored_but_actual_length_is_enforced() {
        let mut buffer = PayloadBuffer::with_capacity_hint(Some(1024), 4).expect("buffer");
        assert!(matches!(buffer.storage, Storage::Heap(ref bytes) if bytes.capacity() == 0));
        buffer.extend_from_slice(b"four").expect("bounded content");
        assert_eq!(
            buffer.extend_from_slice(b"!"),
            Err(PayloadBufferError::TooLarge)
        );
        assert_eq!(buffer.freeze().as_ref(), b"four");
    }

    #[test]
    fn write_interface_moves_large_serialized_output_to_mapped_storage() {
        let mut buffer = PayloadBuffer::new(MAPPED_ALLOCATION_THRESHOLD_BYTES);
        buffer
            .write_all(&vec![b'x'; MAPPED_ALLOCATION_THRESHOLD_BYTES])
            .expect("write mapped content");

        assert!(matches!(buffer.storage, Storage::Mapped(_)));
        let frozen = buffer.freeze();
        assert_eq!(frozen.as_ref().len(), MAPPED_ALLOCATION_THRESHOLD_BYTES);
        assert_eq!(frozen.allocated_bytes(), MAPPED_ALLOCATION_THRESHOLD_BYTES);
    }

    #[test]
    fn write_interface_preserves_the_bounded_length_error() {
        let mut buffer = PayloadBuffer::new(4);
        let error = buffer.write_all(b"fives").expect_err("bounded write");

        assert_eq!(error.kind(), ErrorKind::WriteZero);
        assert!(buffer.is_empty());
    }
}
