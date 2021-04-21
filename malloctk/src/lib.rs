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

pub mod util;
pub mod malloc;

use core::alloc::{GlobalAlloc, Layout};
use std::ptr;
use std::cmp;
use util::Address;


pub trait Plan: GlobalAlloc + Sized + 'static {
    fn new() -> Self;
    fn get_layout(&self, ptr: Address) -> Layout;
}
pub trait Mutator: Sized + 'static {
    fn current() -> &'static mut Self;
    fn get_layout(&self, ptr: Address) -> Layout;
    fn alloc(&mut self, layout: Layout, zeroed: bool) -> Option<Address>;
    fn dealloc(&mut self,  ptr: Address, layout: Layout);
    fn realloc(&mut self, ptr: Address, layout: Layout, new_size: usize) -> Option<Address> {
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new_ptr = self.alloc(new_layout, false);
        if let Some(new_ptr) = new_ptr {
            unsafe {
                ptr::copy_nonoverlapping(ptr.as_ptr::<u8>(), new_ptr.as_mut_ptr::<u8>(), cmp::min(layout.size(), new_size));
            }
            self.dealloc(ptr, layout);
        }
        new_ptr
    }
}


