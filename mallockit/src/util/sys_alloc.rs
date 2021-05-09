use std::{
    alloc::{AllocError, Allocator, Layout},
    intrinsics::unlikely,
    ptr::NonNull,
    slice,
};

pub struct System;

unsafe impl Allocator for System {
    #[inline(always)]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let ptr =
            unsafe { libmimalloc_sys::mi_malloc_aligned(layout.size(), layout.align()) as *mut u8 };
        if unlikely(ptr.is_null()) {
            Err(AllocError)
        } else {
            let slice = unsafe { slice::from_raw_parts_mut(ptr, layout.size()) };
            Ok(NonNull::from(slice))
        }
    }

    #[inline(always)]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, _layout: Layout) {
        libmimalloc_sys::mi_free(ptr.as_ptr() as _);
    }
}
