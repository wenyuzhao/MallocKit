use std::sync::atomic::{AtomicUsize, Ordering};

use mallockit::util::{Address, Lazy, Local};
use spin::{Mutex, MutexGuard};

use crate::{
    block::{Block, BlockExt},
    hoard_space::HoardSpace,
};

pub struct BlockList {
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
    pub fn push_back(&mut self, mut block: Block) {
        debug_assert!(block.prev.is_none());
        debug_assert!(block.next.is_none());
        if let Some(mut tail) = self.tail {
            tail.next = Some(block);
        }
        block.next = None;
        block.prev = self.tail;
        self.tail = Some(block);
        if self.head.is_none() {
            self.head = Some(block)
        }
        debug_assert_ne!(block.prev, Some(block));
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
        while let Some(b) = cursor {
            if b.used_bytes < (Block::BYTES >> 1) {
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
        if cursor.is_none() {
            assert!(self.tail.is_none());
        }
        while let Some(b) = cursor {
            cursor = b.next;
            if cursor.is_none() {
                assert_eq!(
                    self.tail,
                    Some(b),
                    "pool@{:?} {}",
                    pool as *const _,
                    pool.global
                );
            }
        }
        let mut cursor = self.tail;
        if cursor.is_none() {
            assert!(self.head.is_none());
        }
        while let Some(b) = cursor {
            cursor = b.prev;
            if cursor.is_none() {
                assert_eq!(
                    self.head,
                    Some(b),
                    "pool@{:?} {}",
                    pool as *const _,
                    pool.global
                );
            }
        }
        // Check owners
        let mut cursor = self.head;
        while let Some(b) = cursor {
            assert_eq!(b.owner.unwrap() as *const _, pool as *const _, "{:?}", b);
            cursor = b.next;
        }
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

    pub fn push_pack(&self, size_class: usize, mut block: Block) {
        debug_assert!(!block.is_full());
        debug_assert!(block.prev.is_none());
        debug_assert!(block.next.is_none());
        let mut blocks = self.blocks.lock();
        blocks[size_class].push_back(block);
        block.owner = Some(self.static_ref());
        self.used_bytes
            .fetch_add(block.used_bytes, Ordering::Relaxed);
        self.total_bytes.fetch_add(Block::BYTES, Ordering::Relaxed);
        blocks[size_class].verify(self);
    }

    pub fn pop_back(&self, size_class: usize) -> Option<(Block, MutexGuard<[BlockList; 15]>)> {
        debug_assert!(self.global);
        let mut blocks = self.blocks.lock();
        blocks[size_class].verify(self);
        if let Some(mut block) = blocks[size_class].head {
            blocks[size_class].head = block.next;
            if let Some(mut next) = block.next {
                next.prev = None;
            } else {
                debug_assert_eq!(blocks[size_class].tail, Some(block));
                blocks[size_class].tail = None;
            }
            self.used_bytes
                .fetch_sub(block.used_bytes, Ordering::Relaxed);
            self.total_bytes.fetch_sub(Block::BYTES, Ordering::Relaxed);
            debug_assert_eq!(block.owner.unwrap() as *const _, self as *const _);
            block.prev = None;
            block.next = None;
            blocks[size_class].verify(self);
            return Some((block, blocks));
        }
        None
    }

    #[inline(always)]
    pub fn alloc_cell(
        &self,
        size_class: usize,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Option<Address> {
        debug_assert!(!self.global);
        let mut blocks = self.blocks.lock();
        blocks[size_class].verify(self);
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
                debug_assert_ne!(block, b.prev);
                block = b.prev;
            }
            match target {
                Some(block) => block,
                _ => {
                    blocks[size_class].verify(self);
                    // Get a block from global pool
                    let block = space
                        .acquire_block(self, size_class, &mut blocks[size_class])
                        .unwrap();
                    self.used_bytes
                        .fetch_add(block.used_bytes, Ordering::Relaxed);
                    self.total_bytes.fetch_add(Block::BYTES, Ordering::Relaxed);
                    debug_assert!(!block.is_full());
                    blocks[size_class].verify(self);
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
        blocks[size_class].verify(self);
        Some(cell)
    }

    #[inline(always)]
    pub fn free_cell(&self, cell: Address, space: &Lazy<&'static HoardSpace, Local>) {
        let mut owner = self;
        let mut blocks = self.blocks.lock();
        let block = Block::containing(cell);
        while block.owner.unwrap() as *const _ != owner as *const _ {
            owner = block.owner.unwrap();
            std::mem::drop(blocks);
            blocks = owner.blocks.lock();
        }
        owner.free_cell_impl(cell, space, blocks)
    }

    #[inline(always)]
    fn free_cell_impl(
        &self,
        cell: Address,
        space: &Lazy<&'static HoardSpace, Local>,
        mut blocks: MutexGuard<[BlockList; 15]>,
    ) {
        let block = Block::containing(cell);
        let size_class = block.size_class;
        blocks[size_class].verify(self);
        block.free_cell(cell);
        self.used_bytes.fetch_sub(
            HoardSpace::size_class_to_bytes(size_class),
            Ordering::Relaxed,
        );
        blocks[size_class].verify(self);
        if block.is_empty() {
            blocks[size_class].remove(block);
            space.release_block(block);
        } else {
            // Move the block to back
            blocks[size_class].move_to_back(block);
        }
        blocks[size_class].verify(self);
        debug_assert_eq!(block.owner.unwrap() as *const _, self as *const _);
        // Flush?
        if !self.global && self.should_flush() {
            self.free_cell_slow(size_class, space, blocks);
        }
    }

    #[inline(never)]
    fn free_cell_slow(
        &self,
        size_class: usize,
        space: &Lazy<&'static HoardSpace, Local>,
        mut blocks: MutexGuard<[BlockList; 15]>,
    ) {
        // Transit a mostly-empty block to the global pool
        debug_assert!(!self.global);
        if let Some(mostly_empty_block) = blocks[size_class].pop_mostly_empty_block() {
            debug_assert!(!mostly_empty_block.is_full());
            debug_assert_eq!(
                mostly_empty_block.owner.unwrap() as *const _,
                self as *const _,
                "{:?}",
                mostly_empty_block
            );
            self.used_bytes
                .fetch_sub(mostly_empty_block.used_bytes, Ordering::Relaxed);
            self.total_bytes.fetch_sub(Block::BYTES, Ordering::Relaxed);
            space.flush_block(size_class, mostly_empty_block);
            blocks[size_class].verify(self);
            debug_assert_ne!(
                mostly_empty_block.owner.unwrap() as *const _,
                self as *const _
            );
        }
    }
}
