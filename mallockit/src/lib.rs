#![allow(incomplete_features)]
#![feature(core_intrinsics)]
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
#![feature(const_maybe_uninit_assume_init)]
#![feature(const_ptr_write)]
#![feature(associated_type_defaults)]
#![feature(alloc_layout_extra)]
#![feature(const_option)]
#![feature(const_alloc_layout)]
#![feature(asm_const)]
#![feature(adt_const_params)]
#![feature(generic_const_exprs)]
#![feature(type_alias_impl_trait)]
#![feature(specialization)]
#![feature(const_for)]

extern crate mallockit_proc_macro;

extern crate extension_traits;

pub use extension_traits::extension;

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
