use mallockit::util::aligned_block::{AlignedBlock, AlignedBlockConfig};

use crate::{hoard_space::HoardSpace, pool::Pool};

use super::Address;

#[repr(C)]
pub struct BlockMeta {
    pub owner: Option<&'static Pool>,
    pub size_class: usize,
    pub free_bytes: usize,
    pub head_cell: Option<Address>,
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
}

impl BlockExt for Block {
    fn init(mut self, local: &'static Pool, size_class: usize) {
        self.owner = Some(local);
        self.size_class = size_class;
        let size = HoardSpace::size_class_to_bytes(size_class);
        let mut cell = (self.start() + Self::HEADER_BYTES).align_up(size);
        while cell < self.end() {
            unsafe {
                cell.store(self.head_cell.unwrap_or(Address::ZERO));
            }
            self.head_cell = Some(cell);
            cell = cell + size;
        }
    }

    #[inline(always)]
    fn alloc_cell(mut self) -> Option<Address> {
        let cell = self.head_cell?;
        let next = unsafe { cell.load::<Address>() };
        if next.is_zero() {
            self.head_cell = None;
        } else {
            self.head_cell = Some(next);
        }
        self.free_bytes += HoardSpace::size_class_to_bytes(self.size_class);
        Some(cell)
    }

    #[inline(always)]
    fn free_cell(mut self, cell: Address) {
        unsafe {
            cell.store(self.head_cell.unwrap_or(Address::ZERO));
        }
        self.free_bytes -= HoardSpace::size_class_to_bytes(self.size_class);
        self.head_cell = Some(cell);
    }
}
