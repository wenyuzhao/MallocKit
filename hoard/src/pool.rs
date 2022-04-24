use std::sync::atomic::{AtomicUsize, Ordering};

use mallockit::util::{size_class::SizeClass, Address, Lazy, Local};
use spin::{relax::Yield, MutexGuard};

type Mutex<T> = spin::mutex::Mutex<T, Yield>;

use crate::{
    block::{Block, BlockExt},
    hoard_space::HoardSpace,
};

pub type BlockLists = [Mutex<BlockList>; 32];

pub struct BlockList {
    groups: [Option<Block>; Self::GROUPS], // fullnesss groups: <25%, <50%, <75%, <100%, FULL
    used_bytes: AtomicUsize,
    total_bytes: AtomicUsize,
}

impl BlockList {
    const GROUPS: usize = 4 + 1;

    const fn new() -> Self {
        Self {
            groups: [None; Self::GROUPS],
            used_bytes: AtomicUsize::new(0),
            total_bytes: AtomicUsize::new(0),
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
    fn group(block: Block, alloc: bool) -> usize {
        let u = block.used_bytes()
            + if alloc { block.size_class.bytes() } else { 0 }
            + (Address::ZERO + Block::HEADER_BYTES)
                .align_up(block.size_class.bytes())
                .as_usize();
        (u << 2) >> Block::LOG_BYTES
    }

    #[inline(always)]
    fn push(&mut self, mut block: Block, alloc: bool) {
        let group = Self::group(block, alloc);
        block.group = group as _;
        block.next = self.groups[group];
        block.prev = None;
        if let Some(mut head) = self.groups[group] {
            head.prev = Some(block)
        }
        self.groups[group] = Some(block);
        debug_assert_ne!(block.prev, Some(block));
    }

    #[inline(always)]
    fn find(&mut self) -> Option<Block> {
        for i in (0..Self::GROUPS - 1).rev() {
            if let Some(block) = self.groups[i] {
                debug_assert!(!block.is_full());
                return Some(block);
            }
        }
        None
    }

    #[inline(always)]
    fn pop(&mut self) -> Option<Block> {
        for i in (0..Self::GROUPS - 1).rev() {
            if let Some(mut block) = self.groups[i] {
                self.groups[i] = block.next;
                if let Some(mut next) = block.next {
                    next.prev = None;
                }
                block.prev = None;
                block.next = None;
                return Some(block);
            }
        }
        None
    }

    #[inline(always)]
    fn remove(&mut self, block: Block) {
        if self.groups[block.group as usize] == Some(block) {
            self.groups[block.group as usize] = block.next;
        }
        if let Some(mut prev) = block.prev {
            prev.next = block.next;
        }
        if let Some(mut next) = block.next {
            next.prev = block.prev;
        }
    }

    #[inline(always)]
    fn move_to_front(&mut self, mut block: Block, alloc: bool) {
        let group = Self::group(block, alloc);
        let block_group = block.group as usize;
        if Some(block) == self.groups[group] || (alloc && group == block_group) {
            return;
        }
        if self.groups[block_group] == Some(block) {
            self.groups[block_group] = block.next;
        }
        if let Some(mut prev) = block.prev {
            prev.next = block.next;
        }
        if let Some(mut next) = block.next {
            next.prev = block.prev;
        }
        block.group = group as _;
        block.next = self.groups[group];
        block.prev = None;
        if let Some(mut head) = self.groups[group] {
            head.prev = Some(block)
        }
        self.groups[group] = Some(block);
    }

    #[inline(always)]
    fn pop_mostly_empty_block(&mut self) -> Option<Block> {
        for i in 0..Self::GROUPS / 2 {
            if let Some(block) = self.groups[i] {
                self.groups[i] = block.next;
                if let Some(mut next) = block.next {
                    next.prev = None;
                }
                return Some(block);
            }
        }
        None
    }
}

pub struct Pool {
    pub global: bool,
    blocks: BlockLists,
}

impl Pool {
    pub const fn new(global: bool) -> Self {
        const fn b() -> Mutex<BlockList> {
            Mutex::new(BlockList::new())
        }
        Self {
            global,
            blocks: [
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
            ],
        }
    }

    #[inline(always)]
    pub const fn static_ref(&self) -> &'static Self {
        unsafe { &*(self as *const Self) }
    }

    #[inline(always)]
    pub fn push(&self, size_class: SizeClass, mut block: Block) {
        debug_assert!(!block.is_full());
        let mut blocks = self.lock_blocks(size_class);
        blocks.push(block, false);
        block.owner = self.static_ref();
        blocks.used_bytes.store(
            blocks.used_bytes.load(Ordering::Relaxed) + block.used_bytes(),
            Ordering::Relaxed,
        );
        blocks.total_bytes.store(
            blocks.total_bytes.load(Ordering::Relaxed) + Block::BYTES,
            Ordering::Relaxed,
        );
    }

    #[inline(always)]
    pub fn pop(&self, size_class: SizeClass) -> Option<(Block, MutexGuard<BlockList>)> {
        debug_assert!(self.global);
        let mut blocks = self.lock_blocks(size_class);
        if let Some(block) = blocks.pop() {
            blocks.used_bytes.store(
                blocks.used_bytes.load(Ordering::Relaxed) - block.used_bytes(),
                Ordering::Relaxed,
            );
            blocks.total_bytes.store(
                blocks.total_bytes.load(Ordering::Relaxed) - Block::BYTES,
                Ordering::Relaxed,
            );
            debug_assert!(block.is_owned_by(self));
            return Some((block, blocks));
        }
        None
    }

    #[cold]
    pub fn acquire_block_slow(
        &self,
        size_class: SizeClass,
        blocks: &mut MutexGuard<BlockList>,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Block {
        // Get a block from global pool
        let block = space
            .acquire_block(size_class, self, |mut block| {
                blocks.push(block, true);
                block.owner = self.static_ref();
                blocks.used_bytes.store(
                    blocks.used_bytes.load(Ordering::Relaxed)
                        + block.used_bytes()
                        + size_class.bytes(),
                    Ordering::Relaxed,
                );
                blocks.total_bytes.store(
                    blocks.total_bytes.load(Ordering::Relaxed) + Block::BYTES,
                    Ordering::Relaxed,
                );
            })
            .unwrap();
        debug_assert!(!block.is_full());
        block
    }

    #[inline(always)]
    pub fn lock_blocks(&self, size_class: SizeClass) -> MutexGuard<BlockList> {
        self.blocks[size_class.as_usize()].lock()
    }

    #[inline(always)]
    pub fn alloc_cell(
        &self,
        size_class: SizeClass,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Option<Address> {
        debug_assert!(!self.global);
        // Get a local block
        let mut blocks = self.lock_blocks(size_class);
        let block = {
            // Find a mostly-full block
            if let Some(block) = blocks.find() {
                blocks.move_to_front(block, true);
                block
            } else {
                self.acquire_block_slow(size_class, &mut blocks, space)
            }
        };
        // Alloc a cell from the block
        let cell = block.alloc_cell().unwrap();
        blocks.used_bytes.store(
            blocks.used_bytes.load(Ordering::Relaxed) + size_class.bytes(),
            Ordering::Relaxed,
        );
        Some(cell)
    }

    #[inline(always)]
    pub fn free_cell(&self, cell: Address, space: &Lazy<&'static HoardSpace, Local>) {
        let block = Block::containing(cell);
        let mut owner = self;
        let mut blocks = owner.lock_blocks(block.size_class);
        while !block.is_owned_by(owner) {
            std::mem::drop(blocks);
            std::thread::yield_now();
            owner = block.owner;
            blocks = owner.lock_blocks(block.size_class);
        }
        owner.free_cell_impl(cell, space, blocks)
    }

    #[inline(always)]
    fn free_cell_impl(
        &self,
        cell: Address,
        space: &Lazy<&'static HoardSpace, Local>,
        mut blocks: MutexGuard<BlockList>,
    ) {
        let block = Block::containing(cell);
        block.free_cell(cell);
        blocks.used_bytes.store(
            blocks.used_bytes.load(Ordering::Relaxed) - block.size_class.bytes(),
            Ordering::Relaxed,
        );
        if block.is_empty() {
            blocks.remove(block);
            space.release_block(block);
        } else {
            blocks.move_to_front(block, false);
        }
        debug_assert!(block.is_owned_by(self));
        // Flush?
        if !self.global && blocks.should_flush(block.size_class.log_bytes()) {
            self.free_cell_slow(block.size_class, space, blocks);
        }
    }

    #[cold]
    fn free_cell_slow(
        &self,
        size_class: SizeClass,
        space: &Lazy<&'static HoardSpace, Local>,
        mut blocks: MutexGuard<BlockList>,
    ) {
        // Transit a mostly-empty block to the global pool
        debug_assert!(!self.global);
        if let Some(mostly_empty_block) = blocks.pop_mostly_empty_block() {
            debug_assert!(!mostly_empty_block.is_full());
            debug_assert!(mostly_empty_block.is_owned_by(self));
            blocks.used_bytes.store(
                blocks.used_bytes.load(Ordering::Relaxed) - mostly_empty_block.used_bytes(),
                Ordering::Relaxed,
            );
            blocks.total_bytes.store(
                blocks.total_bytes.load(Ordering::Relaxed) - Block::BYTES,
                Ordering::Relaxed,
            );
            space.flush_block(size_class, mostly_empty_block);
            debug_assert!(!mostly_empty_block.is_owned_by(self));
        }
    }
}
