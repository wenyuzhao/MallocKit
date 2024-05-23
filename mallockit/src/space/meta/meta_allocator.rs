use super::META_SPACE;
use crate::util::{Address, LayoutUtils, Page, Size4K};
use std::{
    alloc::{AllocError, Allocator, GlobalAlloc, Layout},
    ptr::NonNull,
    slice,
};

pub(crate) struct MetaLocal {
    freelist: [Address; Self::NUM_SIZE_CLASS],
}

impl MetaLocal {
    const MAX_NON_LARGE_ALLOC_SIZE: usize = Page::<Size4K>::BYTES;
    const NUM_SIZE_CLASS: usize = Page::<Size4K>::LOG_BYTES + 1;

    pub const fn new() -> Self {
        Self {
            freelist: [Address::ZERO; Self::NUM_SIZE_CLASS],
        }
    }

    fn current() -> &'static mut Self {
        &mut crate::mutator::InternalTLS::current().meta
    }

    #[cold]
    fn allocate_large(&self, layout: Layout) -> Result<Address, AllocError> {
        let layout = unsafe { layout.pad_to_align_unchecked() };
        let pages = (layout.size() + Page::<Size4K>::MASK) >> Page::<Size4K>::LOG_BYTES;
        let ptr = META_SPACE
            .map::<Size4K>(pages)
            .ok_or(AllocError)?
            .start
            .start();
        Ok(ptr.align_up(layout.align()))
    }

    #[cold]
    fn release_large(&self, ptr: Address, layout: Layout) {
        let layout = unsafe { layout.pad_to_align_unchecked() };
        let start = Page::<Size4K>::new(ptr);
        let pages = (layout.size() + Page::<Size4K>::MASK) >> Page::<Size4K>::LOG_BYTES;
        META_SPACE.unmap(start, pages)
    }

    const fn request_large(padded_size: usize) -> bool {
        padded_size > Self::MAX_NON_LARGE_ALLOC_SIZE
    }

    fn update_layout(l: Layout) -> Layout {
        let align = usize::max(16, l.align());
        let size = (l.size() + 0b1111) & !0b1111;
        unsafe { Layout::from_size_align_unchecked(size, align) }
    }

    const fn size_class(size: usize) -> usize {
        size.next_power_of_two().trailing_zeros() as _
    }

    fn pop_cell(&mut self, size_class: usize) -> Option<Address> {
        let cell = self.freelist[size_class];
        if !cell.is_zero() {
            let next = unsafe { cell.load::<Address>() };
            self.freelist[size_class] = next;
            Some(cell)
        } else {
            None
        }
    }

    fn push_cell(&mut self, cell: Address, size_class: usize) {
        unsafe { cell.store(self.freelist[size_class]) };
        self.freelist[size_class] = cell;
    }

    #[cold]
    fn allocate_cell_slow(
        &mut self,
        request_size_class: usize,
        retry: bool,
    ) -> Result<Address, AllocError> {
        for size_class in request_size_class..Self::NUM_SIZE_CLASS {
            if let Some(cell) = self.pop_cell(size_class) {
                let parent = cell;
                for parent_size_class in ((request_size_class + 1)..=size_class).rev() {
                    let cell2 = parent + ((1 << parent_size_class) >> 1);
                    let child_size_class = parent_size_class - 1;
                    self.push_cell(cell2, child_size_class);
                }
                return Ok(parent);
            }
        }
        debug_assert!(!retry);
        let cell = META_SPACE.map::<Size4K>(1).ok_or(AllocError)?.start.start();
        self.push_cell(cell, Self::size_class(Page::<Size4K>::BYTES));
        self.allocate_cell_slow(request_size_class, true)
    }

    fn allocate_cell(&mut self, size_class: usize) -> Result<Address, AllocError> {
        if let Some(cell) = self.pop_cell(size_class) {
            Ok(cell)
        } else {
            self.allocate_cell_slow(size_class, false)
        }
    }

    fn allocate(&mut self, layout: Layout) -> Result<Address, AllocError> {
        let layout = Self::update_layout(layout);
        let padded_size = layout.padded_size();
        if !Self::request_large(padded_size) {
            let size_class = Self::size_class(padded_size);
            let cell = self.allocate_cell(size_class)?;
            let addr = cell.align_up(layout.align());
            Ok(addr)
        } else {
            self.allocate_large(layout)
        }
    }

    fn deallocate(&mut self, ptr: Address, layout: Layout) {
        let layout = Self::update_layout(layout);
        let padded_size = layout.padded_size();
        if !Self::request_large(padded_size) {
            let size_class = Self::size_class(padded_size);
            let cell = ptr.align_down(1 << size_class);
            self.push_cell(cell, size_class)
        } else {
            self.release_large(ptr, layout)
        }
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct Meta;

unsafe impl Allocator for Meta {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        let start = MetaLocal::current().allocate(layout)?;
        let slice = unsafe { slice::from_raw_parts_mut(start.as_mut() as *mut u8, layout.size()) };
        Ok(NonNull::from(slice))
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        MetaLocal::current().deallocate(ptr.as_ptr().into(), layout)
    }
}

unsafe impl GlobalAlloc for Meta {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        MetaLocal::current()
            .allocate(layout)
            .unwrap_or(Address::ZERO)
            .into()
    }
    unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
        MetaLocal::current().deallocate(ptr.into(), layout)
    }
}

pub type Box<T> = std::boxed::Box<T, Meta>;
pub type Vec<T> = std::vec::Vec<T, Meta>;
pub type BTreeMap<K, V> = std::collections::BTreeMap<K, V, Meta>;
pub type BTreeSet<V> = std::collections::BTreeSet<V, Meta>;
