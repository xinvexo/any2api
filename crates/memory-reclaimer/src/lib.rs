mod mimalloc;
mod platform;
mod transparent_huge_pages;

pub use mimalloc::mark_current_thread_as_mimalloc_pool_worker;
pub use platform::relieve_native_allocator_pressure;
pub use transparent_huge_pages::disable_for_current_process as disable_transparent_huge_pages;

#[cfg(test)]
mod tests {
    #[test]
    fn allocator_maintenance_is_safe_to_invoke() {
        super::disable_transparent_huge_pages()
            .expect("transparent huge pages should be configurable for this process");
        std::thread::spawn(super::mark_current_thread_as_mimalloc_pool_worker)
            .join()
            .expect("mimalloc thread-pool marker should not panic");
        super::relieve_native_allocator_pressure();
    }
}
