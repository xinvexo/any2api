#![allow(
    unsafe_code,
    reason = "this audited module is the sole Linux transparent-huge-page FFI boundary"
)]

#[cfg(target_os = "linux")]
use std::ffi::{c_int, c_ulong};

#[cfg(target_os = "linux")]
const PR_SET_THP_DISABLE: c_int = 41;

// libmimalloc-sys 0.1.49 embeds mimalloc v3, whose stable source enum assigns
// mi_option_allow_thp this value but whose Rust bindings do not yet export it.
#[cfg(target_os = "linux")]
const MIMALLOC_V3_OPTION_ALLOW_THP: libmimalloc_sys::mi_option_t = 43;

#[cfg(target_os = "linux")]
unsafe extern "C" {
    fn prctl(option: c_int, ...) -> c_int;
}

// Run before mimalloc's priority-101 process constructor. Disabling only from
// Rust main is too late: mimalloc may already have initialized an arena while
// the host-wide THP policy is `always`.
#[cfg(target_os = "linux")]
#[used]
#[unsafe(link_section = ".init_array.00100")]
static DISABLE_THP_BEFORE_MIMALLOC: extern "C" fn() = disable_before_mimalloc_initialization;

#[cfg(target_os = "linux")]
extern "C" fn disable_before_mimalloc_initialization() {
    // SAFETY: ELF constructors run serially here, before mimalloc's process
    // initializer. The pinned mimalloc v3 option is a process-wide boolean.
    unsafe {
        libmimalloc_sys::mi_option_set_enabled(MIMALLOC_V3_OPTION_ALLOW_THP, false);
    }
    let _ = request_kernel_disable();
}

#[cfg(target_os = "linux")]
fn request_kernel_disable() -> c_int {
    let enabled: c_ulong = 1;
    let zero: c_ulong = 0;
    // SAFETY: PR_SET_THP_DISABLE accepts one unsigned-long boolean followed by
    // unused zero arguments and changes only the calling process' kernel flag.
    unsafe { prctl(PR_SET_THP_DISABLE, enabled, zero, zero, zero) }
}

#[cfg(target_os = "linux")]
pub fn disable_for_current_process() -> std::io::Result<()> {
    if request_kernel_disable() == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

#[cfg(not(target_os = "linux"))]
pub fn disable_for_current_process() -> std::io::Result<()> {
    Ok(())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::MIMALLOC_V3_OPTION_ALLOW_THP;

    #[test]
    fn process_initializer_disables_the_mimalloc_thp_option() {
        // SAFETY: reading an initialized mimalloc option is valid after the
        // single-threaded ELF constructor phase has completed.
        let enabled =
            unsafe { libmimalloc_sys::mi_option_is_enabled(MIMALLOC_V3_OPTION_ALLOW_THP) };
        assert!(!enabled);
    }
}
