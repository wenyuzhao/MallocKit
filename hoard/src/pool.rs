use std::sync::atomic::{AtomicUsize, Ordering};

use mallockit::util::{Address, Lazy, Local};
use spin::Mutex;

use crate::{
    block::{Block, BlockExt},
    hoard_space::HoardSpace,
};

struct BlockList {
    head: Option<Block>,
    tail: Option<Block>,
    // used_bytes: usize,
}

impl BlockList {
    const fn new() -> Self {
        Self {
            head: None,
            tail: None,
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
        if Some(block) == self.tail {
            return;
        }
        self.remove(block);
        self.push_back(block);
    }

    #[inline(always)]
    fn pop_mostly_empty_block(&mut self) -> Option<Block> {
        let mut cursor = self.head;
        while let Some(mut b) = cursor {
            if b.used_bytes < (Block::BYTES >> 1) {
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
    pub used_bytes: AtomicUsize,
    pub total_bytes: AtomicUsize,
    blocks: Mutex<[BlockList; HoardSpace::size_class(Block::BYTES)]>,
}

impl Pool {
    pub const fn new(global: bool) -> Self {
        const fn b() -> BlockList {
            BlockList::new()
        }
        Self {
            global,
            used_bytes: AtomicUsize::new(0),
            total_bytes: AtomicUsize::new(0),
            blocks: Mutex::new([
                b(),
                b(),
                b(),
                b(),
                b(),
                b(),
                b(),
                b(),
                b(),
                b(),
                b(),
                b(),
                b(),
                b(),
                b(),
            ]),
        }
    }

    #[inline(always)]
    fn should_flush(&self) -> bool {
        const EMPTINESS_CLASSES: usize = 8;
        let u = self.used_bytes.load(Ordering::Relaxed);
        let a = self.total_bytes.load(Ordering::Relaxed);
        u + (2 * Block::BYTES) < a && (EMPTINESS_CLASSES * u) < ((EMPTINESS_CLASSES - 1) * a)
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
            self.used_bytes
                .fetch_sub(block.used_bytes, Ordering::Relaxed);
            self.total_bytes.fetch_sub(Block::BYTES, Ordering::Relaxed);
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
                if !b.is_full() {
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
                    self.used_bytes
                        .fetch_add(block.used_bytes, Ordering::Relaxed);
                    self.total_bytes.fetch_add(Block::BYTES, Ordering::Relaxed);
                    block
                }
            }
        };
        // Alloc a cell from the block
        let cell = block.alloc_cell().unwrap();
        self.used_bytes.fetch_add(
            HoardSpace::size_class_to_bytes(size_class),
            Ordering::Relaxed,
        );
        Some(cell)
    }

    #[inline(always)]
    pub fn free_cell(&self, cell: Address, space: &Lazy<&'static HoardSpace, Local>) {
        let mut blocks = self.blocks.lock();
        let block = Block::containing(cell);
        let size_class = block.size_class;
        block.free_cell(cell);
        self.used_bytes.fetch_sub(
            HoardSpace::size_class_to_bytes(size_class),
            Ordering::Relaxed,
        );
        if block.is_empty() {
            blocks[size_class].remove(block);
            space.release_block(block);
        } else {
            // Move the block to back
            blocks[size_class].move_to_back(block);
        }
        // Flush?
        if !self.global && self.should_flush() {
            self.free_cell_slow(size_class, space, &mut *blocks);
        }
    }

    #[inline(never)]
    fn free_cell_slow(
        &self,
        size_class: usize,
        space: &Lazy<&'static HoardSpace, Local>,
        blocks: &mut [BlockList],
    ) {
        // Transit a mostly-empty block to the global pool
        debug_assert!(!self.global);
        if let Some(mostly_empty_block) = blocks[size_class].pop_mostly_empty_block() {
            self.used_bytes
                .fetch_sub(mostly_empty_block.used_bytes, Ordering::Relaxed);
            self.total_bytes.fetch_sub(Block::BYTES, Ordering::Relaxed);
            space.flush_block(size_class, mostly_empty_block);
        }
    }
}
