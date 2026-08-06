mod platform;

pub use platform::reclaim_process_memory;

#[cfg(test)]
mod tests {
    #[test]
    fn native_reclaimer_is_safe_to_invoke() {
        super::reclaim_process_memory();
    }
}
