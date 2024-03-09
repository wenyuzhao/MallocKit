#![allow(incomplete_features)]
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]
#![feature(step_trait)]
#![feature(thread_local)]
#![feature(allocator_api)]
#![feature(never_type)]
#![feature(associated_type_defaults)]
#![feature(alloc_layout_extra)]
#![feature(adt_const_params)]
#![feature(generic_const_exprs)]
#![feature(effects)]

extern crate mallockit_proc_macro;

#[macro_use]
pub mod log;
#[macro_use]
pub mod util;
#[doc(hidden)]
pub mod hooks;
pub mod malloc;
pub mod mutator;
pub mod plan;
pub mod space;
pub mod stat;
pub mod testing;
pub mod worker;

pub use ctor::ctor;
pub use libc;
pub use mallockit_proc_macro::*;
pub use mutator::Mutator;
pub use plan::Plan;
use space::meta::Meta;

#[cfg(not(target_pointer_width = "64"))]
const ERROR: ! = "32-bit is not supported";

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
)))]
const ERROR: ! = r#"
    ❌ Unsupported Platform.
    Only the following platforms are supported:
        Linux (x86_64), macOS (x86_64), Linux (aarch64).
"#;

#[global_allocator]
static META: Meta = Meta;
