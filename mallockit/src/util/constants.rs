#[cfg(not(any(
    target_os = "macos",
    all(target_os = "windows", target_pointer_width = "64")
)))]
pub const LOG_MIN_ALIGNMENT: usize = 4; // should be 8?
#[cfg(any(
    target_os = "macos",
    all(target_os = "windows", target_pointer_width = "64")
))]
pub const LOG_MIN_ALIGNMENT: usize = 4;

pub const MIN_ALIGNMENT: usize = 1 << LOG_MIN_ALIGNMENT;
