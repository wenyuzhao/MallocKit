use std::intrinsics::unlikely;

use mallockit::util::{
    aligned_block::{AlignedBlock, AlignedBlockConfig},
    size_class::SizeClass,
};

use crate::pool::Pool;

use super::Address;

#[repr(C)]
pub struct BlockMeta {
    bump_cursor: u32,
    used_bytes: u32,
    pub prev: Option<Block>,
    pub next: Option<Block>,
    pub size_class: SizeClass,
    pub group: u8,
    head_cell: Address,
    pub owner: &'static Pool,
}

pub struct BlockConfig;

impl AlignedBlockConfig for BlockConfig {
    const LOG_BYTES: usize = 18;
    type Header = BlockMeta;
}

pub type Block = AlignedBlock<BlockConfig>;

pub trait BlockExt: Sized {
    const DATA_BYTES: usize = Block::BYTES - Block::HEADER_BYTES;
    fn init(self, local: &'static Pool, sc: SizeClass);
    fn alloc_cell(self) -> Option<Address>;
    fn free_cell(self, cell: Address);
    fn is_empty(self) -> bool;
    fn is_full(self) -> bool;
    fn is_owned_by(self, owner: &Pool) -> bool;
    fn used_bytes(self) -> usize;
}

impl BlockExt for Block {
    #[inline(always)]
    fn init(mut self, _local: &'static Pool, size_class: SizeClass) {
        debug_assert_eq!(Block::HEADER_BYTES, Address::BYTES * 6);
        self.size_class = size_class;
        let size = size_class.bytes();
        self.head_cell = Address::ZERO;
        self.bump_cursor = (Address::ZERO + Self::HEADER_BYTES)
            .align_up(size)
            .as_usize() as u32;
        self.used_bytes = 0;
    }

    #[inline(always)]
    fn used_bytes(self) -> usize {
        self.used_bytes as _
    }

    #[inline(always)]
    fn is_empty(self) -> bool {
        self.used_bytes == 0
    }

    #[inline(always)]
    fn is_full(self) -> bool {
        self.bump_cursor >= Self::BYTES as u32 && self.head_cell.is_zero()
    }

    #[inline(always)]
    fn alloc_cell(mut self) -> Option<Address> {
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
    fn free_cell(mut self, cell: Address) {
        unsafe {
            cell.store(self.head_cell);
        }
        self.head_cell = cell;
        self.used_bytes -= self.size_class.bytes() as u32;
    }

    #[inline(always)]
    fn is_owned_by(self, owner: &Pool) -> bool {
        self.owner as *const _ == owner as *const _
    }
}
