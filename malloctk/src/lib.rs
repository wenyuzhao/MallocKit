#![feature(core_intrinsics)]
#![feature(const_fn)]
#![feature(maybe_uninit_extra)]
#![feature(const_fn_fn_ptr_basics)]

pub mod lazy;
pub mod malloc;

use core::alloc::{GlobalAlloc, Layout};


pub trait Plan: GlobalAlloc + Sized + 'static {
    fn new() -> Self;
    fn get_layout(&self, ptr: *mut u8) -> Layout;
}
