#![allow(incomplete_features)]
#![feature(core_intrinsics)]
#![feature(const_fn)]
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
#![feature(step_trait_ext)]
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

#[macro_use] extern crate derive_more;

#[macro_use]
pub mod log;
#[macro_use]
pub mod util;
pub mod space;
pub mod malloc;
#[doc(hidden)]
pub mod hooks;

use core::alloc::Layout;
use std::ptr;
use std::cmp;
use util::Address;
pub use ctor::ctor;
pub use libc;


pub trait Plan: Sized + 'static {
    type Mutator: Mutator<Plan=Self>;

    fn new() -> Self;
    fn init(&self) {}
    fn get_layout(&self, ptr: Address) -> Layout;
}

pub trait Mutator: Sized + 'static {
    type Plan: Plan<Mutator=Self>;

    fn current() -> &'static mut Self;
    fn plan(&self) -> &'static Self::Plan;

    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        self.plan().get_layout(ptr)
    }

    fn alloc(&mut self, layout: Layout) -> Option<Address>;

    #[inline(always)]
    fn alloc_zeroed(&mut self, layout: Layout) -> Option<Address> {
        let size = layout.size();
        let ptr = self.alloc(layout);
        if let Some(ptr) = ptr {
            unsafe { ptr::write_bytes(ptr.as_mut_ptr::<u8>(), 0, size) };
        }
        ptr
    }

    fn dealloc(&mut self, ptr: Address);

    #[inline(always)]
    fn realloc(&mut self, ptr: Address, new_size: usize) -> Option<Address> {
        let layout = self.get_layout(ptr);
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new_ptr = self.alloc(new_layout);
        if let Some(new_ptr) = new_ptr {
            unsafe {
                ptr::copy_nonoverlapping(ptr.as_ptr::<u8>(), new_ptr.as_mut_ptr::<u8>(), cmp::min(layout.size(), new_size));
            }
            self.dealloc(ptr);
        }
        new_ptr
    }
}
