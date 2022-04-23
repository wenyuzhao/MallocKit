use std::sync::atomic::{AtomicUsize, Ordering};

use mallockit::util::{size_class::SizeClass, Address, Lazy, Local};
use spin::{Mutex, MutexGuard};

use crate::{
    block::{Block, BlockExt},
    hoard_space::HoardSpace,
};

pub type BlockLists = [BlockList; 32];

pub struct BlockList {
    head: Option<Block>,
}

impl BlockList {
    const fn new() -> Self {
        Self { head: None }
    }

    #[inline(always)]
    fn push(&mut self, mut block: Block) {
        debug_assert!(block.prev.is_none());
        debug_assert!(block.next.is_none());
        block.next = self.head;
        block.prev = None;
        if let Some(mut head) = self.head {
            head.prev = Some(block)
        }
        self.head = Some(block);
        debug_assert_ne!(block.prev, Some(block));
    }

    #[inline(always)]
    fn remove(&mut self, mut block: Block) {
        if self.head == Some(block) {
            self.head = block.next;
        }
        if let Some(mut prev) = block.prev {
            prev.next = block.next;
        }
        if let Some(mut next) = block.next {
            next.prev = block.prev;
        }
        block.prev = None;
        block.next = None;
    }

    #[inline(always)]
    fn move_to_front(&mut self, block: Block) {
        if Some(block) == self.head {
            return;
        }
        self.remove(block);
        self.push(block);
    }

    #[inline(always)]
    fn pop_mostly_empty_block(&mut self) -> Option<Block> {
        let mut cursor = self.head;
        while let Some(b) = cursor {
            if b.used_bytes() < (Block::BYTES >> 1) {
                self.remove(b);
                return Some(b);
            }
            cursor = b.next;
        }
        None
    }

    #[inline(always)]
    fn verify(&self, pool: &Pool) {
        if !cfg!(debug_assertions) {
            return;
        }
        let mut cursor = self.head;
        while let Some(b) = cursor {
            assert!(b.is_owned_by(pool));
            cursor = b.next;
        }
    }
}

pub struct Pool {
    pub global: bool,
    pub used_bytes: AtomicUsize,
    pub total_bytes: AtomicUsize,
    blocks: Mutex<BlockLists>,
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
                b(),
                b(),
            ]),
        }
    }

    #[inline(always)]
    fn should_flush(&self, log_obj_size: usize) -> bool {
        const EMPTINESS_CLASSES: usize = 8;
        let u = self.used_bytes.load(Ordering::Relaxed);
        let a = self.total_bytes.load(Ordering::Relaxed);
        u + ((2 * Block::BYTES) >> log_obj_size) < a
            && (EMPTINESS_CLASSES * u) < ((EMPTINESS_CLASSES - 1) * a)
    }

    #[inline(always)]
    pub const fn static_ref(&self) -> &'static Self {
        unsafe { &*(self as *const Self) }
    }

    #[inline(always)]
    pub fn push(&self, size_class: SizeClass, mut block: Block) {
        debug_assert!(!block.is_full());
        debug_assert!(block.prev.is_none());
        debug_assert!(block.next.is_none());
        let mut blocks = self.blocks.lock();
        blocks[size_class.as_usize()].push(block);
        block.owner = self.static_ref();
        self.used_bytes
            .fetch_add(block.used_bytes(), Ordering::Relaxed);
        self.total_bytes.fetch_add(Block::BYTES, Ordering::Relaxed);
        blocks[size_class.as_usize()].verify(self);
    }

    #[inline(always)]
    pub fn pop(&self, size_class: SizeClass) -> Option<(Block, MutexGuard<BlockLists>)> {
        debug_assert!(self.global);
        let mut blocks = self.blocks.lock();
        let index = size_class.as_usize();
        blocks[index].verify(self);
        if let Some(mut block) = blocks[index].head {
            blocks[index].head = block.next;
            if let Some(mut next) = block.next {
                next.prev = None;
            }
            self.used_bytes
                .fetch_sub(block.used_bytes(), Ordering::Relaxed);
            self.total_bytes.fetch_sub(Block::BYTES, Ordering::Relaxed);
            debug_assert!(block.is_owned_by(self));
            block.prev = None;
            block.next = None;
            blocks[index].verify(self);
            return Some((block, blocks));
        }
        None
    }

    #[cold]
    pub fn acquire_block_slow(
        &self,
        size_class: SizeClass,
        blocks: &mut MutexGuard<BlockLists>,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Block {
        let index = size_class.as_usize();
        blocks[index].verify(self);
        // Get a block from global pool
        let block = space
            .acquire_block(size_class, self, |mut block| {
                blocks[index].push(block);
                block.owner = self.static_ref();
            })
            .unwrap();
        self.used_bytes
            .fetch_add(block.used_bytes(), Ordering::Relaxed);
        self.total_bytes.fetch_add(Block::BYTES, Ordering::Relaxed);
        debug_assert!(!block.is_full());
        blocks[index].verify(self);
        block
    }

    #[inline(always)]
    pub fn alloc_cell(
        &self,
        size_class: SizeClass,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Option<Address> {
        debug_assert!(!self.global);
        let mut blocks = self.blocks.lock();
        let index = size_class.as_usize();
        blocks[index].verify(self);
        // Get a local block
        let block = {
            // Go through the list reversely to find a non-full block
            let mut target = None;
            let mut block = blocks[index].head;
            while let Some(b) = block {
                if !b.is_full() {
                    target = Some(b);
                    break;
                }
                debug_assert_ne!(block, b.next);
                block = b.next;
            }
            match target {
                Some(block) => block,
                _ => self.acquire_block_slow(size_class, &mut blocks, space),
            }
        };
        // Alloc a cell from the block
        let cell = block.alloc_cell().unwrap();
        self.used_bytes.store(
            self.used_bytes.load(Ordering::Relaxed) + size_class.bytes(),
            Ordering::Relaxed,
        );
        blocks[index].verify(self);
        Some(cell)
    }

    #[inline(always)]
    pub fn free_cell(&self, cell: Address, space: &Lazy<&'static HoardSpace, Local>) {
        let mut owner = self;
        let mut blocks = self.blocks.lock();
        let block = Block::containing(cell);
        while !block.is_owned_by(owner) {
            std::mem::drop(blocks);
            owner = block.owner;
            blocks = owner.blocks.lock();
        }
        owner.free_cell_impl(cell, space, blocks)
    }

    #[inline(always)]
    fn free_cell_impl(
        &self,
        cell: Address,
        space: &Lazy<&'static HoardSpace, Local>,
        mut blocks: MutexGuard<BlockLists>,
    ) {
        let block = Block::containing(cell);
        let index = block.size_class.as_usize();
        blocks[index].verify(self);
        block.free_cell(cell);
        self.used_bytes.store(
            self.used_bytes.load(Ordering::Relaxed) - block.size_class.bytes(),
            Ordering::Relaxed,
        );
        blocks[index].verify(self);
        if block.is_empty() {
            blocks[index].remove(block);
            space.release_block(block);
        } else {
            // Move the block to front
            blocks[index].move_to_front(block);
        }
        blocks[index].verify(self);
        debug_assert!(block.is_owned_by(self));
        // Flush?
        if !self.global && self.should_flush(block.size_class.log_bytes()) {
            self.free_cell_slow(block.size_class, space, blocks);
        }
    }

    #[inline(never)]
    fn free_cell_slow(
        &self,
        size_class: SizeClass,
        space: &Lazy<&'static HoardSpace, Local>,
        mut blocks: MutexGuard<BlockLists>,
    ) {
        // Transit a mostly-empty block to the global pool
        debug_assert!(!self.global);
        let index = size_class.as_usize();
        if let Some(mostly_empty_block) = blocks[index].pop_mostly_empty_block() {
            debug_assert!(!mostly_empty_block.is_full());
            debug_assert!(mostly_empty_block.is_owned_by(self));
            self.used_bytes
                .fetch_sub(mostly_empty_block.used_bytes(), Ordering::Relaxed);
            self.total_bytes.fetch_sub(Block::BYTES, Ordering::Relaxed);
            space.flush_block(size_class, mostly_empty_block);
            blocks[index].verify(self);
            debug_assert!(!mostly_empty_block.is_owned_by(self));
        }
    }
}
