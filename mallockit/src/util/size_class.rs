use std::{alloc::Layout, usize};

use super::LayoutUtils;

#[repr(transparent)]
#[derive(Debug, Clone, Copy)]
pub struct SizeClass<const LOG_COVERAGE: u8 = 4>(pub u8);

impl<const LOG_COVERAGE: u8> SizeClass<LOG_COVERAGE> {
    #[inline(always)]
    pub const fn as_usize(self) -> usize {
        self.0 as usize
    }

    #[inline(always)]
    pub const fn log_bytes(self) -> usize {
        self.0 as usize + LOG_COVERAGE as usize
    }

    #[inline(always)]
    pub const fn bytes(self) -> usize {
        1 << self.log_bytes()
    }

    #[inline(always)]
    pub fn layout(self) -> Layout {
        let size = 1usize << (self.0 + LOG_COVERAGE);
        Layout::from_size_align(size, size).unwrap()
    }

    #[inline(always)]
    pub const fn from_bytes(bytes: usize) -> Self {
        Self(bytes.trailing_zeros() as u8 - LOG_COVERAGE)
    }

    #[inline(always)]
    pub const fn from_layout(layout: Layout) -> Self {
        let layout = unsafe { layout.pad_to_align_unchecked() };
        let size = layout.size().next_power_of_two();
        Self::from_bytes(size)
    }
}
