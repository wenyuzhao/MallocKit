use std::{alloc::Layout, intrinsics::likely};
use std::mem;
use super::Address;


#[derive(Debug)]
pub struct AllocationArea {
    pub top: Address,
    pub limit: Address,
}

impl AllocationArea {
    pub const EMPTY: Self = Self {
        top: Address::ZERO,
        limit: Address::ZERO,
    };

    pub const fn align_up(value: usize, align: usize) -> usize {
        let mask = align - 1;
        (value + mask) & !mask
    }

    pub const fn align_allocation(start: Address, align: usize) -> Address {
        let mask = align - 1;
        Address::from((*start + mask) & !mask)
    }

    #[inline(always)]
    pub const fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let top = self.top;
        let start = Self::align_allocation(top, layout.align());
        let end = start + layout.size();
        if likely(usize::from(end) <= usize::from(self.limit)) {
            self.top = end;
            Some(start)
        } else {
            None
        }
    }

    const fn get_layout_slot(ptr: Address) -> &'static mut (u32, u32) {
        debug_assert!(mem::size_of::<(u32, u32)>() == mem::size_of::<usize>());
        unsafe { (ptr - mem::size_of::<usize>()).as_mut::<(u32, u32)>() }
    }

    pub const fn alloc_with_layout(&mut self, layout: Layout) -> Option<Address> {
        debug_assert!(layout.align() >= std::mem::size_of::<usize>());
        let new_layout = unsafe { Layout::from_size_align_unchecked(layout.size() + layout.align(), layout.align()) };
        let top = self.top;
        let start = Self::align_allocation(top, new_layout.align());
        let end = start + new_layout.size();
        if likely(usize::from(end) <= usize::from(self.limit)) {
            let data_start = start + layout.align();
            *Self::get_layout_slot(data_start) = (layout.size() as u32, layout.align() as u32);
            self.top = end;
            Some(data_start)
        } else {
            None
        }
    }

    pub const fn load_layout(ptr: Address) -> Layout {
        let (size, align) = *Self::get_layout_slot(ptr);
        unsafe { Layout::from_size_align_unchecked(size as _, align as _) }
    }
}

