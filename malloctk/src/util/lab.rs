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

    #[inline(always)]
    pub const fn alloc_with_layout(&mut self, layout: Layout) -> Option<Address> {
        let top = self.top + mem::size_of::<Layout>();
        let start = Self::align_allocation(top, layout.align());
        let end = start + layout.size();
        if likely(usize::from(end) <= usize::from(self.limit)) {
            self.top = end;
            unsafe { *(start - mem::size_of::<Layout>()).as_mut::<Layout>() = layout };
            Some(start)
        } else {
            None
        }
    }

    #[inline(always)]
    pub const fn load_layout(ptr: Address) -> Layout {
        unsafe { *ptr.as_ptr::<Layout>().sub(1) }
    }
}

