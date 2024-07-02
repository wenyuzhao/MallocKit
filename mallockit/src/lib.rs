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
#![feature(asm_const)]
#![feature(const_refs_to_cell)]
#![feature(const_refs_to_static)]

extern crate mallockit_macros;
pub extern crate spin;

#[macro_use]
pub mod util;
pub mod mutator;
pub mod plan;
pub mod space;
pub mod stat;
pub mod worker;

pub use ctor::ctor;
pub use libc;
pub use mallockit_macros::*;
pub use mutator::Mutator;
pub use plan::Plan;

#[cfg(not(target_pointer_width = "64"))]
const ERROR: ! = "32-bit is not supported";

#[cfg(not(any(
    all(target_os = "linux", target_arch = "x86_64"),
    all(target_os = "linux", target_arch = "aarch64"),
    all(target_os = "macos", target_arch = "x86_64"),
    all(target_os = "macos", target_arch = "aarch64"),
)))]
const ERROR: ! = r#"
    ❌ Unsupported Platform.
    Only the following platforms are supported:
        Linux (x86_64), macOS (x86_64), Linux (aarch64).
"#;
