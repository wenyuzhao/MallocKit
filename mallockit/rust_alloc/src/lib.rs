#![feature(allocator_api)]

use std::alloc::{GlobalAlloc, Layout};

pub struct MallocKit;

#[cfg_attr(feature = "buddy", link(name = "buddy"))]
#[cfg_attr(feature = "bump", link(name = "bump"))]
#[cfg_attr(feature = "hoard", link(name = "hoard"))]
#[cfg_attr(feature = "sanity", link(name = "sanity"))]
#[allow(improper_ctypes)]
extern "C" {
    fn mallockit_alloc(layout: std::alloc::Layout) -> *mut u8;
    fn mallockit_dealloc(ptr: *mut u8, layout: std::alloc::Layout);
    fn mallockit_realloc(ptr: *mut u8, layout: std::alloc::Layout, new_size: usize) -> *mut u8;
}

unsafe impl GlobalAlloc for MallocKit {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        mallockit_alloc(layout)
    }

    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        mallockit_dealloc(ptr, layout)
    }

    unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
        mallockit_realloc(ptr, layout, new_size)
    }
}
