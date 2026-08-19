mod mimalloc;
mod platform;

pub use mimalloc::mark_current_thread_as_mimalloc_pool_worker;
pub use platform::relieve_native_allocator_pressure;

#[cfg(test)]
mod tests {
    #[test]
    fn allocator_maintenance_is_safe_to_invoke() {
        std::thread::spawn(super::mark_current_thread_as_mimalloc_pool_worker)
            .join()
            .expect("mimalloc thread-pool marker should not panic");
        super::relieve_native_allocator_pressure();
    }
}
