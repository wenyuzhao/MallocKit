#![allow(incomplete_features)]
#![feature(impl_trait_in_bindings)]
#![feature(min_type_alias_impl_trait)]
#![feature(core_intrinsics)]
#![feature(const_fn)]
#![feature(const_raw_ptr_to_usize_cast)]

use core::{alloc::{GlobalAlloc, Layout},  ptr};
use malloctk::*;

static mut DATA: [u8; 1 << 22] = [0u8; 1 << 22];
static mut CURSOR: *mut u8 = ptr::null_mut();
static mut LIMIT: *mut u8 = ptr::null_mut();

struct Bump;

unsafe impl GlobalAlloc for Bump {
    #[inline(always)]
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        // let _guard = LOCK.lock();
        let size = layout.size() + core::mem::size_of::<Layout>();
        let align = layout.align();
        if CURSOR.is_null() {
            CURSOR = &mut DATA[0];
            LIMIT = &mut DATA[DATA.len() - 1];
        }
        let start = ((CURSOR as usize).wrapping_add(align).wrapping_sub(1) & !align.wrapping_sub(1)) as *mut u8;
        let end = start as usize + size;
        CURSOR = end as *mut u8;
        *(start as *mut Layout) = layout;
        return (start as *mut u8).add(core::mem::size_of::<Layout>())
    }
    unsafe fn dealloc(&self, _ptr: *mut u8, _layout: Layout) {}
}

impl Plan for Bump {
    fn new() -> Self {
        Self
    }

    #[inline(always)]
    fn get_layout(&self, ptr: *mut u8) -> Layout {
        unsafe { *(ptr as *mut Layout).sub(1) }
    }
}

export_malloc_api!(Bump);