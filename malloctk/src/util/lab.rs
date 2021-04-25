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
    pub fn alloc(&mut self, layout: Layout, with_layout: bool) -> Option<Address> {
        let top = self.top + if with_layout { mem::size_of::<Layout>() } else { 0 };
        let start = Self::align_allocation(top, layout.align());
        let end = start + layout.size();
        if likely(end <= self.limit) {
            self.top = end;
            if with_layout {
                unsafe { (start - mem::size_of::<Layout>()).store(layout) };
            }
            Some(start)
        } else {
            None
        }
    }
}

