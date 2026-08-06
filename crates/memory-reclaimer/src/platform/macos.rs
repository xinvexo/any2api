#![allow(
    unsafe_code,
    reason = "this audited module is the sole macOS allocator FFI boundary"
)]

unsafe extern "C" {
    fn malloc_zone_pressure_relief(zone: *mut std::ffi::c_void, goal: usize) -> usize;
}

pub fn reclaim_process_memory() {
    // SAFETY: a null zone asks libSystem to inspect every malloc zone, and a
    // zero goal requests maximal best-effort relief. No Rust pointer is shared.
    let _ = unsafe { malloc_zone_pressure_relief(std::ptr::null_mut(), 0) };
}
