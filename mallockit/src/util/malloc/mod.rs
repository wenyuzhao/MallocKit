#[cfg(target_os = "macos")]
pub mod macos_malloc_zone;
#[macro_use]
mod malloc_api;

pub use malloc_api::*;
