#![allow(
    unsafe_code,
    reason = "this audited module is the sole Windows heap FFI boundary"
)]

use windows_sys::Win32::System::{
    Memory::{HeapOptimizeResources, HeapSetInformation},
    SystemServices::{
        HEAP_OPTIMIZE_RESOURCES_CURRENT_VERSION, HEAP_OPTIMIZE_RESOURCES_INFORMATION,
    },
};

pub fn reclaim_process_memory() {
    let _ = optimize_process_heaps();
}

fn optimize_process_heaps() -> bool {
    let information = HEAP_OPTIMIZE_RESOURCES_INFORMATION {
        Version: HEAP_OPTIMIZE_RESOURCES_CURRENT_VERSION,
        Flags: 0,
    };
    // SAFETY: the documented null heap form applies HeapOptimizeResources to
    // every LFH heap. windows-sys supplies the ABI layout and current version;
    // `information` remains immutably borrowed for the duration of the call.
    (unsafe {
        HeapSetInformation(
            std::ptr::null_mut(),
            HeapOptimizeResources,
            std::ptr::from_ref(&information).cast(),
            std::mem::size_of::<HEAP_OPTIMIZE_RESOURCES_INFORMATION>(),
        )
    }) != 0
}

#[cfg(test)]
mod tests {
    #[test]
    fn windows_accepts_the_versioned_heap_optimizer_parameters() {
        assert!(super::optimize_process_heaps());
    }
}
