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
pub use linux_gnu::reclaim_process_memory;
#[cfg(target_os = "macos")]
pub use macos::reclaim_process_memory;
#[cfg(not(any(
    all(target_os = "linux", target_env = "gnu"),
    target_os = "macos",
    target_os = "windows"
)))]
pub use noop::reclaim_process_memory;
#[cfg(target_os = "windows")]
pub use windows::reclaim_process_memory;
