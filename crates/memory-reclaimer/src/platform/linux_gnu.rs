#![allow(
    unsafe_code,
    reason = "this audited module is the sole GNU allocator FFI boundary"
)]

unsafe extern "C" {
    fn malloc_trim(pad: usize) -> std::ffi::c_int;
}

pub fn relieve_native_allocator_pressure() {
    // SAFETY: malloc_trim is a process-wide, thread-safe glibc allocator API;
    // zero is a valid pad and no pointer crosses the FFI boundary.
    let _ = unsafe { malloc_trim(0) };
}
