use super::META_SPACE;
use crate::util::{Page, Size4K};
use std::{
    alloc::{AllocError, Allocator, Layout},
    ptr::NonNull,
    slice,
};

pub(crate) struct MetaLocal;

impl MetaLocal {
    pub const fn new() -> Self {
        Self
    }

    #[inline(always)]
    pub fn current() -> &'static mut Self {
        &mut crate::mutator::InternalTLS::current().meta
    }
}

unsafe impl Allocator for MetaLocal {
    #[inline(always)]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        MetaLocal::current().allocate(layout)
    }

    #[inline(always)]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        MetaLocal::current().deallocate(ptr, layout)
    }
}

pub struct Meta;

unsafe impl Allocator for Meta {
    #[inline(always)]
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let pages = (layout.size() + Page::<Size4K>::MASK) >> Page::<Size4K>::LOG_BYTES;
        let start = META_SPACE
            .map::<Size4K>(pages)
            .ok_or(AllocError)?
            .start
            .start();
        let slice = unsafe { slice::from_raw_parts_mut(start.as_mut() as *mut u8, layout.size()) };
        Ok(NonNull::from(slice))
    }

    #[inline(always)]
    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        let start = Page::<Size4K>::new(ptr.as_ptr().into());
        let pages = (layout.size() + Page::<Size4K>::MASK) >> Page::<Size4K>::LOG_BYTES;
        META_SPACE.unmap(start, pages)
    }
}
