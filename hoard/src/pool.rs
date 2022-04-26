use std::{
    intrinsics::likely,
    sync::atomic::{AtomicUsize, Ordering},
};

use mallockit::util::{size_class::SizeClass, Address, Lazy, Local};
use spin::{relax::Yield, MutexGuard};

type Mutex<T> = spin::mutex::Mutex<T, Yield>;

use crate::{
    block::{Block, BlockExt},
    hoard_space::HoardSpace,
};

pub type BlockLists = [Mutex<BlockList>; 32];

pub struct BlockList {
    cache: Option<Block>,
    groups: [Option<Block>; Self::GROUPS], // fullnesss groups: <25%, <50%, <75%, <100%, FULL
    used_bytes: AtomicUsize,
    total_bytes: AtomicUsize,
}

impl BlockList {
    const GROUPS: usize = 4 + 1;

    const fn new() -> Self {
        Self {
            cache: None,
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
    fn push(&mut self, mut block: Block, alloc: bool, update_stats: bool) {
        if alloc && self.cache.is_some() {
            let cache = self.cache.unwrap();
            self.cache = Some(block);
            block.group = u8::MAX;
            block = cache;
        }
        let group = Self::group(block, alloc);
        block.group = group as _;
        block.next = self.groups[group];
        block.prev = None;
        if let Some(mut head) = self.groups[group] {
            head.prev = Some(block)
        }
        self.groups[group] = Some(block);
        debug_assert_ne!(block.prev, Some(block));
        if update_stats {
            self.inc_used_bytes(block.used_bytes());
            self.inc_total_bytes(Block::BYTES);
        }
    }

    #[cold]
    fn find_slow(&mut self) -> Option<Block> {
        if let Some(block) = self.cache {
            self.cache = None;
            self.push(block, false, false)
        }
        for i in (0..Self::GROUPS - 1).rev() {
            if let Some(mut block) = self.groups[i] {
                debug_assert!(!block.is_full());
                self.remove(block, false);
                self.cache = Some(block);
                block.group = u8::MAX;
                return Some(block);
            }
        }
        None
    }

    #[inline(always)]
    fn find(&mut self) -> Option<Block> {
        if let Some(block) = self.cache {
            if !block.is_full() {
                return Some(block);
            }
        }
        self.find_slow()
    }

    #[inline(always)]
    fn pop(&mut self) -> Option<Block> {
        if let Some(block) = self.cache.take() {
            return Some(block);
        }
        for i in (0..Self::GROUPS - 1).rev() {
            if let Some(block) = self.groups[i] {
                self.groups[i] = block.next;
                if let Some(mut next) = block.next {
                    next.prev = None;
                }
                self.dec_used_bytes(block.used_bytes());
                self.dec_total_bytes(Block::BYTES);
                return Some(block);
            }
        }
        None
    }

    #[inline(always)]
    fn remove(&mut self, block: Block, update_stats: bool) {
        if self.cache == Some(block) {
            self.cache = None;
            return;
        }
        if self.groups[block.group as usize] == Some(block) {
            self.groups[block.group as usize] = block.next;
        }
        if let Some(mut prev) = block.prev {
            prev.next = block.next;
        }
        if let Some(mut next) = block.next {
            next.prev = block.prev;
        }
        if update_stats {
            self.dec_used_bytes(block.used_bytes());
            self.dec_total_bytes(Block::BYTES);
        }
    }

    #[cold]
    fn move_to_front_slow(&mut self, mut block: Block, alloc: bool) {
        if likely(Some(block) == self.cache) {
            return;
        }
        let group = Self::group(block, alloc);
        let block_group = block.group as usize;
        if likely(Some(block) == self.groups[group] || group == block_group) {
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
    fn move_to_front(&mut self, block: Block, alloc: bool) {
        if likely(Some(block) == self.cache) {
            return;
        }
        self.move_to_front_slow(block, alloc)
    }

    #[inline(always)]
    fn pop_mostly_empty_block(&mut self) -> Option<Block> {
        for i in 0..Self::GROUPS / 2 {
            if let Some(block) = self.groups[i] {
                self.groups[i] = block.next;
                if let Some(mut next) = block.next {
                    next.prev = None;
                }
                self.dec_used_bytes(block.used_bytes());
                self.dec_total_bytes(Block::BYTES);
                return Some(block);
            }
        }
        None
    }

    #[inline(always)]
    fn inc_used_bytes(&self, used_bytes: usize) {
        self.used_bytes.store(
            self.used_bytes.load(Ordering::Relaxed) + used_bytes,
            Ordering::Relaxed,
        )
    }

    #[inline(always)]
    fn dec_used_bytes(&self, used_bytes: usize) {
        self.used_bytes.store(
            self.used_bytes.load(Ordering::Relaxed) - used_bytes,
            Ordering::Relaxed,
        )
    }

    #[inline(always)]
    fn inc_total_bytes(&self, total_bytes: usize) {
        self.total_bytes.store(
            self.total_bytes.load(Ordering::Relaxed) + total_bytes,
            Ordering::Relaxed,
        )
    }

    #[inline(always)]
    fn dec_total_bytes(&self, total_bytes: usize) {
        self.total_bytes.store(
            self.total_bytes.load(Ordering::Relaxed) - total_bytes,
            Ordering::Relaxed,
        )
    }
}

pub struct Pool {
    pub global: bool,
    blocks: BlockLists,
    cache: [Address; 32],
    local_bytes: usize,
}

impl Pool {
    pub const fn new(global: bool) -> Self {
        const fn b() -> Mutex<BlockList> {
            Mutex::new(BlockList::new())
        }
        Self {
            global,
            cache: [Address::ZERO; 32],
            local_bytes: 0,
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
        block.owner = self.static_ref();
        blocks.push(block, false, true);
    }

    #[inline(always)]
    pub fn pop(&self, size_class: SizeClass) -> Option<(Block, MutexGuard<BlockList>)> {
        debug_assert!(self.global);
        let mut blocks = self.lock_blocks(size_class);
        if let Some(block) = blocks.pop() {
            debug_assert!(block.is_owned_by(self));
            return Some((block, blocks));
        }
        None
    }

    #[cold]
    pub fn acquire_block_slow(
        &self,
        size_class: SizeClass,
        blocks: &mut BlockList,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Block {
        // Get a block from global pool
        let block = space
            .acquire_block(size_class, self, |mut block| {
                block.owner = self.static_ref();
                blocks.push(block, true, true);
            })
            .unwrap();
        debug_assert!(!block.is_full());
        block
    }

    #[inline(always)]
    fn add_to_cache(&mut self, size_class: SizeClass, cell: Address) {
        unsafe { cell.store(self.cache[size_class.as_usize()]) };
        self.cache[size_class.as_usize()] = cell;
        self.local_bytes += size_class.bytes();
    }

    #[inline(always)]
    fn remove_from_cache(&mut self, size_class: SizeClass) -> Option<Address> {
        let cell = self.cache[size_class.as_usize()];
        if cell.is_zero() {
            return None;
        }
        self.cache[size_class.as_usize()] = unsafe { cell.load() };
        self.local_bytes -= size_class.bytes();
        return Some(cell);
    }

    #[inline(always)]
    pub fn lock_blocks(&self, size_class: SizeClass) -> MutexGuard<BlockList> {
        self.blocks[size_class.as_usize()].lock()
    }

    #[inline(always)]
    pub fn alloc_cell(
        &mut self,
        size_class: SizeClass,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Option<Address> {
        debug_assert!(!self.global);
        if let Some(cell) = self.remove_from_cache(size_class) {
            return Some(cell);
        }
        self.alloc_cell_slow(size_class, space)
    }

    #[cold]
    fn alloc_cell_slow(
        &mut self,
        size_class: SizeClass,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Option<Address> {
        debug_assert!(!self.global);
        let mut blocks = self.blocks[size_class.as_usize()].lock();
        let block = if let Some(block) = blocks.find() {
            blocks.move_to_front(block, true);
            block
        } else {
            self.acquire_block_slow(size_class, &mut blocks, space)
        };
        let cell = block.alloc_cell().unwrap();
        blocks.inc_used_bytes(size_class.bytes());
        // if let Some(cell) = block.alloc_cell() {
        //     Self::add_to_cache(&mut self.cache, block.size_class, cell)
        // }
        Some(cell)
    }

    const LOCAL_HEAP_THRESHOLD: usize = 16 * 1024 * 1024;
    const LARGEST_SMALL_OBJECT: usize = 1024;

    #[inline(always)]
    pub fn free_cell(&mut self, cell: Address, space: &Lazy<&'static HoardSpace, Local>) {
        let block = Block::containing(cell);
        let size = block.size_class.bytes();
        if likely(
            size <= Self::LARGEST_SMALL_OBJECT
                && size + self.local_bytes <= Self::LOCAL_HEAP_THRESHOLD,
        ) {
            self.add_to_cache(block.size_class, cell);
        } else {
            self.free_cell_slow(cell, space);
        }
    }

    #[cold]
    fn free_cell_slow(&self, cell: Address, space: &Lazy<&'static HoardSpace, Local>) {
        let block = Block::containing(cell);
        let mut owner = block.owner;
        let mut blocks = owner.lock_blocks(block.size_class);
        while !block.is_owned_by(owner) {
            std::mem::drop(blocks);
            std::thread::yield_now();
            owner = block.owner;
            blocks = owner.lock_blocks(block.size_class);
        }
        owner.free_cell_slow_impl(cell, space, &mut blocks)
    }

    #[inline(always)]
    fn free_cell_slow_impl(
        &self,
        cell: Address,
        space: &Lazy<&'static HoardSpace, Local>,
        blocks: &mut BlockList,
    ) {
        let block = Block::containing(cell);
        block.free_cell(cell);
        blocks.dec_used_bytes(block.size_class.bytes());
        if block.is_empty() {
            blocks.remove(block, true);
            space.release_block(block);
        } else {
            blocks.move_to_front(block, false);
        }
        debug_assert!(block.is_owned_by(self));
        // Flush?
        if !self.global && blocks.should_flush(block.size_class.log_bytes()) {
            self.flush_block_slow(block.size_class, space, blocks);
        }
    }

    #[cold]
    fn flush_block_slow(
        &self,
        size_class: SizeClass,
        space: &Lazy<&'static HoardSpace, Local>,
        blocks: &mut BlockList,
    ) {
        // Transit a mostly-empty block to the global pool
        debug_assert!(!self.global);
        if let Some(mostly_empty_block) = blocks.pop_mostly_empty_block() {
            debug_assert!(!mostly_empty_block.is_full());
            debug_assert!(mostly_empty_block.is_owned_by(self));
            space.flush_block(size_class, mostly_empty_block);
            debug_assert!(!mostly_empty_block.is_owned_by(self));
        }
    }
}
