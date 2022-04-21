use mallockit::util::{Address, Lazy, Local};
use spin::Mutex;

use crate::{
    block::{Block, BlockExt},
    hoard_space::HoardSpace,
};

struct BlockList {
    head: Option<Block>,
    tail: Option<Block>,
    total_bytes: usize,
    used_bytes: usize,
}

impl BlockList {
    const fn new() -> Self {
        Self {
            head: None,
            tail: None,
            total_bytes: 0,
            used_bytes: 0,
        }
    }

    #[inline(always)]
    fn push_back(&mut self, mut block: Block) {
        if let Some(mut tail) = self.tail {
            tail.next = Some(block);
        }
        block.next = None;
        block.prev = self.tail;
        self.tail = Some(block);
        if self.head.is_none() {
            self.head = Some(block)
        }
    }

    #[inline(always)]
    fn remove(&mut self, mut block: Block) {
        let prev = block.prev;
        let next = block.next;
        if let Some(mut prev) = prev {
            prev.next = next;
        }
        if let Some(mut next) = next {
            next.prev = prev;
        }
        if self.head == Some(block) {
            self.head = next;
        }
        if self.tail == Some(block) {
            self.tail = prev;
        }
        block.prev = None;
        block.next = None;
    }

    #[inline(always)]
    fn move_to_back(&mut self, block: Block) {
        self.remove(block);
        self.push_back(block);
    }

    #[inline(always)]
    fn should_flush(&self) -> bool {
        (self.used_bytes * 100 / self.total_bytes) < 10
    }

    #[inline(always)]
    fn pop_mostly_empty_block(&mut self) -> Option<Block> {
        let mut cursor = self.head;
        while let Some(mut b) = cursor {
            if b.free_bytes >= (Block::BYTES >> 1) {
                self.remove(b);
                b.owner = None;
                return Some(b);
            }
            cursor = b.next;
        }
        None
    }
}

pub struct Pool {
    pub global: bool,
    blocks: Mutex<[BlockList; 9]>,
}

impl Pool {
    pub const fn new(global: bool) -> Self {
        const fn b() -> BlockList {
            BlockList::new()
        }
        Self {
            global,
            blocks: Mutex::new([b(), b(), b(), b(), b(), b(), b(), b(), b()]),
        }
    }

    pub const fn static_ref(&self) -> &'static Self {
        unsafe { &*(self as *const Self) }
    }

    pub fn push_pack(&self, size_class: usize, block: Block) {
        debug_assert!(self.global);
        let mut blocks = self.blocks.lock();
        blocks[size_class].push_back(block);
    }

    pub fn pop_back(&self, size_class: usize) -> Option<Block> {
        debug_assert!(self.global);
        let mut blocks = self.blocks.lock();
        if let Some(block) = blocks[size_class].head {
            let next = block.next;
            blocks[size_class].head = next;
            if next.is_none() {
                blocks[size_class].tail = None;
            }
            return Some(block);
        }
        None
    }

    #[inline(always)]
    pub fn alloc_cell(
        &self,
        size_class: usize,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Option<Address> {
        let mut blocks = self.blocks.lock();
        // Get a local block
        let block = {
            // Go through the list reversely to find a non-full block
            let mut target = None;
            let mut block = blocks[size_class].tail;
            while let Some(b) = block {
                if b.head_cell.is_some() {
                    target = Some(b);
                    break;
                }
                block = b.prev;
            }
            match target {
                Some(block) => block,
                _ => {
                    // Get a block from global pool
                    let block = space.acquire_block(self, size_class).unwrap();
                    blocks[size_class].push_back(block);
                    blocks[size_class].total_bytes += Block::DATA_BYTES;
                    blocks[size_class].used_bytes += Block::DATA_BYTES - block.free_bytes;
                    block
                }
            }
        };
        // Alloc a cell from the block
        let cell = block.alloc_cell().unwrap();
        blocks[size_class].used_bytes += HoardSpace::size_class_to_bytes(size_class);
        Some(cell)
    }

    #[inline(always)]
    pub fn free_cell(&self, cell: Address, space: &Lazy<&'static HoardSpace, Local>) {
        let mut blocks = self.blocks.lock();
        let block = Block::containing(cell);
        let size_class = block.size_class;
        block.free_cell(cell);
        blocks[size_class].used_bytes -= HoardSpace::size_class_to_bytes(size_class);
        // Move the block to back
        blocks[size_class].move_to_back(block);
        // Flush?
        if !self.global && blocks[size_class].should_flush() {
            // Find a mostly-empty block
            debug_assert!(!self.global);
            if let Some(mostly_empty_block) = blocks[size_class].pop_mostly_empty_block() {
                blocks[size_class].total_bytes -= Block::DATA_BYTES;
                blocks[size_class].used_bytes -= Block::DATA_BYTES - block.free_bytes;
                space.flush_block(size_class, mostly_empty_block);
            }
        }
    }
}
