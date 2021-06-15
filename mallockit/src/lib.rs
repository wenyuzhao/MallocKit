#![allow(incomplete_features)]
#![feature(core_intrinsics)]
#![feature(maybe_uninit_extra)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_trait_impl)]
#![feature(const_raw_ptr_to_usize_cast)]
#![feature(const_fn_transmute)]
#![feature(const_raw_ptr_deref)]
#![feature(const_mut_refs)]
#![feature(const_panic)]
#![feature(const_ptr_is_null)]
#![feature(type_ascription)]
#![feature(step_trait)]
#![feature(const_likely)]
#![feature(thread_local)]
#![feature(allocator_api)]
#![feature(never_type)]
#![feature(box_syntax)]
#![feature(const_ptr_offset)]
#![feature(const_maybe_uninit_assume_init)]
#![feature(const_ptr_write)]
#![feature(const_maybe_uninit_as_ptr)]
#![feature(const_generics)]
#![feature(const_evaluatable_checked)]
#![feature(associated_type_defaults)]
#![feature(const_fn_trait_bound)]
#![feature(const_generics_defaults)]
#![feature(asm)]

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

pub use ctor::ctor;
pub use libc;
pub use mallockit_proc_macro::*;
pub use mutator::Mutator;
pub use plan::Plan;

#[cfg(not(target_pointer_width = "64"))]
const ERROR: ! = "32-bit is not supported";

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const ERROR: ! = "Unsupported OS";

#[cfg(not(target_arch = "x86_64"))]
const ERROR: ! = "Unsupported Architecture";
