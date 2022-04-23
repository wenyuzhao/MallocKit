use mallockit::util::aligned_block::{AlignedBlock, AlignedBlockConfig};

use crate::{hoard_space::HoardSpace, pool::Pool};

use super::Address;

#[repr(C)]
pub struct BlockMeta {
    pub owner: Option<&'static Pool>,
    pub size_class: usize,
    pub used_bytes: usize,
    head_cell: Address,
    pub prev: Option<Block>,
    pub next: Option<Block>,
}

pub struct BlockConfig;

impl AlignedBlockConfig for BlockConfig {
    const LOG_BYTES: usize = 18;
    type Header = BlockMeta;
}

pub type Block = AlignedBlock<BlockConfig>;

pub trait BlockExt: Sized {
    const DATA_BYTES: usize = Block::BYTES - Block::HEADER_BYTES;
    fn init(self, local: &'static Pool, sc: usize);
    fn alloc_cell(self) -> Option<Address>;
    fn free_cell(self, cell: Address);
    fn is_empty(self) -> bool;
    fn is_full(self) -> bool;
}

impl BlockExt for Block {
    fn init(mut self, _local: &'static Pool, size_class: usize) {
        self.owner = None;
        self.size_class = size_class;
        let size = HoardSpace::size_class_to_bytes(size_class);
        self.head_cell = Address::ZERO;
        let mut cell = (self.start() + Self::HEADER_BYTES).align_up(size);
        while cell < self.end() {
            unsafe {
                cell.store(self.head_cell);
            }
            self.head_cell = cell;
            cell = cell + size;
        }
        self.used_bytes = 0;
        self.prev = None;
        self.next = None;
    }

    #[inline(always)]
    fn is_empty(self) -> bool {
        self.used_bytes == 0
    }

    #[inline(always)]
    fn is_full(self) -> bool {
        self.head_cell.is_zero()
    }

    #[inline(always)]
    fn alloc_cell(mut self) -> Option<Address> {
        let cell = if self.head_cell.is_zero() {
            return None;
        } else {
            self.head_cell
        };
        self.head_cell = unsafe { cell.load::<Address>() };
        self.used_bytes += HoardSpace::size_class_to_bytes(self.size_class);
        Some(cell)
    }

    #[inline(always)]
    fn free_cell(mut self, cell: Address) {
        unsafe {
            cell.store(self.head_cell);
        }
        self.head_cell = cell;
        self.used_bytes -= HoardSpace::size_class_to_bytes(self.size_class);
    }
}
