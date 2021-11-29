#![allow(incomplete_features)]
#![feature(core_intrinsics)]
#![feature(maybe_uninit_extra)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(const_trait_impl)]
#![feature(const_mut_refs)]
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
#![feature(associated_type_defaults)]
#![feature(const_fn_trait_bound)]
#![feature(const_generics_defaults)]
#![feature(asm)]
#![feature(alloc_layout_extra)]
#![feature(const_option)]
#![feature(const_alloc_layout)]
#![feature(asm_const)]
#![feature(adt_const_params)]
#![feature(generic_const_exprs)]
#![feature(type_alias_impl_trait)]

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

#[cfg(not(any(target_os = "linux", target_os = "macos")))]
const ERROR: ! = "Unsupported OS";

#[cfg(not(target_arch = "x86_64"))]
const ERROR: ! = "Unsupported Architecture";

#[global_allocator]
static META: Meta = Meta;
