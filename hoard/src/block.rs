use mallockit::util::aligned_block::{AlignedBlock, AlignedBlockConfig};

use crate::hoard_space::HoardLocal;

use super::Address;

#[repr(C)]
pub struct BlockMeta {
    pub owner: Option<&'static HoardLocal>,
    pub size_class: usize,
    pub free_bytes: usize,
    pub head_cell: Option<Address>,
    pub next: Option<Block>,
}

pub struct BlockConfig;

impl AlignedBlockConfig for BlockConfig {
    const LOG_BYTES: usize = 12;
    type Header = BlockMeta;
}

pub type Block = AlignedBlock<BlockConfig>;

pub trait BlockExt {
    const DATA_BYTES: usize = Block::BYTES - Block::HEADER_BYTES;
    fn init(self, local: &'static HoardLocal, sc: usize);
    fn alloc_cell(self) -> Option<Address>;
    fn free_cell(self, cell: Address);
}

impl BlockExt for Block {
    fn init(mut self, local: &'static HoardLocal, sc: usize) {
        self.owner = Some(local);
        self.size_class = sc;
        let size = 1usize << (sc + 3);
        let mut cell = (self.start() + Self::HEADER_BYTES).align_up(size);
        while cell < self.end() {
            self.free_cell(cell);
            cell = cell + (1 << (sc + 3));
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
        self.free_bytes += 1usize << (self.size_class + 3);
        Some(cell)
    }

    #[inline(always)]
    fn free_cell(mut self, cell: Address) {
        unsafe {
            cell.store(self.head_cell.unwrap_or(Address::ZERO));
        }
        self.free_bytes -= 1usize << (self.size_class + 3);
        self.head_cell = Some(cell);
    }
}

// //
