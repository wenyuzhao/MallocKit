use crate::util::LayoutUtils;

use super::Address;
use std::mem;
use std::ops::Range;
use std::{alloc::Layout, intrinsics::likely};

#[derive(Debug)]
pub struct AllocationArea {
    pub top: Address,
    pub limit: Address,
}

type Header = (u32, u32);

impl AllocationArea {
    pub const HEADER: Layout = Layout::new::<Header>();
    pub const EMPTY: Self = Self {
        top: Address::ZERO,
        limit: Address::ZERO,
    };

    pub const fn align_up(value: usize, align: usize) -> usize {
        let mask = align - 1;
        (value + mask) & !mask
    }

    pub const fn align_allocation(start: Address, align: usize) -> Address {
        start.align_up(align)
    }

    #[inline(always)]
    pub const fn refill(&mut self, top: Address, limit: Address) {
        self.top = top;
        self.limit = limit;
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

    const fn get_layout_slot(ptr: Address) -> &'static mut Header {
        debug_assert!(mem::size_of::<Header>() == mem::size_of::<usize>());
        unsafe { (ptr - mem::size_of::<usize>()).as_mut::<Header>() }
    }

    #[inline(always)]
    pub fn alloc_assume_aligned(&mut self, layout: Layout) -> Option<Address> {
        debug_assert!(layout.align() >= std::mem::size_of::<usize>());
        debug_assert_eq!(self.top, Self::align_allocation(self.top, layout.align()));
        let start = self.top;
        let end = start + layout.size();
        if likely(usize::from(end) <= usize::from(self.limit)) {
            self.top = end;
            Some(start)
        } else {
            None
        }
    }

    #[inline(always)]
    pub fn alloc_with_layout_assume_aligned(&mut self, layout: Layout) -> Option<Address> {
        debug_assert!(layout.align() >= std::mem::size_of::<usize>());
        let (new_layout, _) = unsafe { Self::HEADER.extend_unchecked(layout) };
        debug_assert_eq!(
            self.top,
            Self::align_allocation(self.top, new_layout.align())
        );
        self.alloc_with_layout(layout)
    }

    #[inline(always)]
    pub fn alloc_with_layout(&mut self, layout: Layout) -> Option<Address> {
        debug_assert!(layout.align() >= std::mem::size_of::<usize>());
        let (new_layout, offset) = unsafe { Self::HEADER.extend_unchecked(layout) };
        let top = self.top;
        let start = Self::align_allocation(top, new_layout.align());
        let end = start + new_layout.size();
        if likely(end <= self.limit) {
            let data_start = start + offset;
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

    #[inline(always)]
    pub fn load_range(ptr: Address) -> Range<Address> {
        let (size, align) = *Self::get_layout_slot(ptr);
        let (new_layout, offset) = unsafe {
            Self::HEADER.extend_unchecked(Layout::from_size_align_unchecked(size as _, align as _))
        };
        let start = ptr - offset;
        let end = start + new_layout.size();
        start..end
    }
}