#![allow(
    unsafe_code,
    reason = "this module is the audited zstd custom allocator callback boundary"
)]

use std::{
    alloc::{Layout, alloc, dealloc},
    collections::{HashMap, hash_map::Entry},
    ffi::c_void,
    ptr::{self, NonNull},
    sync::{
        Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

use memmap2::{MmapMut, MmapOptions};

const MAPPED_ALLOCATION_THRESHOLD_BYTES: usize = 256 * 1024;
const ZSTD_ALLOCATION_ALIGNMENT: usize = 16;

enum Allocation {
    Heap {
        pointer: NonNull<u8>,
        layout: Layout,
    },
    Mapped(MmapMut),
}

impl Drop for Allocation {
    fn drop(&mut self) {
        if let Self::Heap { pointer, layout } = self {
            // SAFETY: `pointer` was returned by `alloc` for this exact layout and
            // remains owned by this allocation until drop.
            unsafe { dealloc(pointer.as_ptr(), *layout) };
        }
    }
}

pub(super) struct AllocatorState {
    allocations: Mutex<HashMap<usize, Allocation>>,
    failed: AtomicBool,
}

impl AllocatorState {
    pub(super) fn new() -> Self {
        Self {
            allocations: Mutex::new(HashMap::new()),
            failed: AtomicBool::new(false),
        }
    }

    pub(super) fn custom_memory(&mut self) -> zstd_sys::ZSTD_customMem {
        zstd_sys::ZSTD_customMem {
            customAlloc: Some(allocate),
            customFree: Some(free),
            opaque: (self as *mut Self).cast::<c_void>(),
        }
    }

    pub(super) fn begin_operation(&self) {
        self.failed.store(false, Ordering::Release);
    }

    pub(super) fn allocation_failed(&self) -> bool {
        self.failed.load(Ordering::Acquire)
    }

    #[cfg(test)]
    pub(super) fn has_mapped_allocation(&self) -> bool {
        self.allocations
            .lock()
            .expect("allocator lock")
            .values()
            .any(|allocation| matches!(allocation, Allocation::Mapped(_)))
    }
}

unsafe extern "C" fn allocate(opaque: *mut c_void, size: usize) -> *mut c_void {
    let Some(opaque) = NonNull::new(opaque.cast::<AllocatorState>()) else {
        return ptr::null_mut();
    };
    // SAFETY: zstd receives this pointer from `AllocatorState::custom_memory`;
    // the owning box outlives the zstd context and all callbacks.
    let state = unsafe { opaque.as_ref() };
    let allocation = if size >= MAPPED_ALLOCATION_THRESHOLD_BYTES {
        MmapOptions::new()
            .len(size)
            .map_anon()
            .map(Allocation::Mapped)
    } else {
        allocate_heap(state, size)
    };
    let Ok(mut allocation) = allocation else {
        state.failed.store(true, Ordering::Release);
        return ptr::null_mut();
    };
    let pointer = match &mut allocation {
        Allocation::Heap { pointer, .. } => pointer.as_ptr(),
        Allocation::Mapped(mapping) => mapping.as_mut_ptr(),
    };
    let Ok(mut allocations) = state.allocations.lock() else {
        state.failed.store(true, Ordering::Release);
        return ptr::null_mut();
    };
    match allocations.entry(pointer as usize) {
        Entry::Vacant(entry) => {
            entry.insert(allocation);
            pointer.cast()
        }
        Entry::Occupied(_) => {
            state.failed.store(true, Ordering::Release);
            ptr::null_mut()
        }
    }
}

fn allocate_heap(state: &AllocatorState, size: usize) -> std::io::Result<Allocation> {
    let layout = Layout::from_size_align(size.max(1), ZSTD_ALLOCATION_ALIGNMENT).map_err(|_| {
        state.failed.store(true, Ordering::Release);
        std::io::Error::other("invalid zstd allocation layout")
    })?;
    // SAFETY: the validated non-zero layout is retained with the pointer and
    // supplied unchanged to `dealloc` by `Allocation::drop`.
    let pointer = NonNull::new(unsafe { alloc(layout) }).ok_or_else(|| {
        state.failed.store(true, Ordering::Release);
        std::io::Error::other("zstd heap allocation failed")
    })?;
    Ok(Allocation::Heap { pointer, layout })
}

unsafe extern "C" fn free(opaque: *mut c_void, address: *mut c_void) {
    let Some(opaque) = NonNull::new(opaque.cast::<AllocatorState>()) else {
        return;
    };
    let Some(address) = NonNull::new(address.cast::<u8>()) else {
        return;
    };
    // SAFETY: zstd receives this pointer from `AllocatorState::custom_memory`;
    // the owning box is dropped only after `ZSTD_freeDStream` returns.
    let state = unsafe { opaque.as_ref() };
    if let Ok(mut allocations) = state.allocations.lock() {
        allocations.remove(&(address.as_ptr() as usize));
    }
}
