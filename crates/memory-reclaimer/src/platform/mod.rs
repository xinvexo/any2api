#[cfg(all(target_os = "linux", target_env = "gnu"))]
mod linux_gnu;
#[cfg(target_os = "macos")]
mod macos;
#[cfg(not(any(
    all(target_os = "linux", target_env = "gnu"),
    target_os = "macos",
    target_os = "windows"
)))]
mod noop;
#[cfg(target_os = "windows")]
mod windows;

#[cfg(all(target_os = "linux", target_env = "gnu"))]
pub use linux_gnu::relieve_native_allocator_pressure;
#[cfg(target_os = "macos")]
pub use macos::relieve_native_allocator_pressure;
#[cfg(not(any(
    all(target_os = "linux", target_env = "gnu"),
    target_os = "macos",
    target_os = "windows"
)))]
pub use noop::relieve_native_allocator_pressure;
#[cfg(target_os = "windows")]
pub use windows::relieve_native_allocator_pressure;
