#![allow(
    unsafe_code,
    reason = "this audited module is the sole mimalloc lifecycle FFI boundary"
)]

unsafe extern "C" {
    fn mi_thread_set_in_threadpool();
}

pub fn mark_current_thread_as_mimalloc_pool_worker() {
    // SAFETY: mimalloc documents this idempotent, no-argument call for
    // initializing a thread before it enters allocator-backed work.
    unsafe { libmimalloc_sys::mi_thread_init() };
    // SAFETY: mimalloc owns thread-local allocator state and exposes this
    // no-argument call specifically for long-lived thread-pool workers.
    unsafe { mi_thread_set_in_threadpool() };
}

pub(crate) fn collect_unused_pages() {
    // SAFETY: mimalloc documents this process allocator maintenance call as
    // thread-safe. `true` asks it to purge unused pages aggressively; no Rust
    // pointer crosses the FFI boundary.
    unsafe { libmimalloc_sys::mi_collect(true) };
}
