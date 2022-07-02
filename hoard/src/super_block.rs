use std::{intrinsics::unlikely, num::NonZeroUsize};

use mallockit::util::{aligned_block::AlignedBlockConfig, size_class::SizeClass};

use crate::pool::Pool;

use super::Address;

#[repr(C)]
pub struct BlockMeta {
    bump_cursor: u32,
    used_bytes: u32,
    pub prev: Option<SuperBlock>,
    pub next: Option<SuperBlock>,
    pub size_class: SizeClass,
    pub group: u8,
    head_cell: Address,
    pub owner: &'static Pool,
}

#[mallockit::aligned_block]
pub struct SuperBlock(NonZeroUsize);

impl const AlignedBlockConfig for SuperBlock {
    type Header = BlockMeta;
    const LOG_BYTES: usize = 18;

    #[inline(always)]
    fn from_address(address: Address) -> Self {
        debug_assert!(!address.is_zero());
        debug_assert!(Self::is_aligned(address));
        Self(unsafe { NonZeroUsize::new_unchecked(usize::from(address)) })
    }

    #[inline(always)]
    fn into_address(self) -> Address {
        Address::from(self.0.get())
    }
}

impl SuperBlock {
    #[inline(always)]
    pub fn init(mut self, _local: &'static Pool, size_class: SizeClass) {
        debug_assert_eq!(SuperBlock::HEADER_BYTES, Address::BYTES * 6);
        self.size_class = size_class;
        let size = size_class.bytes();
        self.head_cell = Address::ZERO;
        self.bump_cursor = (Address::ZERO + Self::HEADER_BYTES)
            .align_up(size)
            .as_usize() as u32;
        self.used_bytes = 0;
    }

    #[inline(always)]
    pub const fn used_bytes(self) -> usize {
        self.used_bytes as _
    }

    #[inline(always)]
    pub const fn is_empty(self) -> bool {
        self.used_bytes == 0
    }

    #[inline(always)]
    pub const fn is_full(self) -> bool {
        self.bump_cursor >= Self::BYTES as u32 && self.head_cell.is_zero()
    }

    #[inline(always)]
    pub const fn alloc_cell(mut self) -> Option<Address> {
        let cell = if unlikely(self.head_cell.is_zero()) {
            if self.bump_cursor < Self::BYTES as u32 {
                let cell = self.start() + (self.bump_cursor as usize);
                self.bump_cursor = self.bump_cursor + self.size_class.bytes() as u32;
                self.used_bytes += self.size_class.bytes() as u32;
                return Some(cell);
            } else {
                return None;
            }
        } else {
            self.head_cell
        };
        self.head_cell = unsafe { cell.load::<Address>() };
        self.used_bytes += self.size_class.bytes() as u32;
        Some(cell)
    }

    #[inline(always)]
    pub const fn free_cell(mut self, cell: Address) {
        unsafe {
            cell.store(self.head_cell);
        }
        self.head_cell = cell;
        self.used_bytes -= self.size_class.bytes() as u32;
    }

    #[inline(always)]
    pub fn is_owned_by(self, owner: &Pool) -> bool {
        self.owner as *const _ == owner as *const _
    }
}
