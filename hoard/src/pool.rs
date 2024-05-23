use crate::{hoard_space::HoardSpace, super_block::SuperBlock};
use array_const_fn_init::array_const_fn_init;
use mallockit::{
    space::page_resource::Block,
    util::{mem::size_class::SizeClass, Address, Lazy, Local},
};
use spin::{relax::Yield, MutexGuard};
use std::sync::atomic::{AtomicUsize, Ordering};

type Mutex<T> = spin::mutex::Mutex<T, Yield>;

pub struct BlockList {
    cache: Option<SuperBlock>,
    groups: [Option<SuperBlock>; Self::GROUPS], // fullnesss groups: <25%, <50%, <75%, <100%, FULL
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

    fn should_flush(&self, log_obj_size: usize) -> bool {
        const EMPTINESS_CLASSES: usize = 8;
        let u = self.used_bytes.load(Ordering::Relaxed);
        let a = self.total_bytes.load(Ordering::Relaxed);
        u + ((2 * SuperBlock::BYTES) >> log_obj_size) < a
            && (EMPTINESS_CLASSES * u) < ((EMPTINESS_CLASSES - 1) * a)
    }

    fn group(block: SuperBlock, alloc: bool) -> usize {
        let u = block.used_bytes()
            + if alloc { block.size_class.bytes() } else { 0 }
            + (Address::ZERO + SuperBlock::META_BYTES)
                .align_up(block.size_class.bytes())
                .as_usize();
        (u << 2) >> SuperBlock::LOG_BYTES
    }

    fn push(&mut self, mut block: SuperBlock, alloc: bool, update_stats: bool) {
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
            self.inc_total_bytes(SuperBlock::BYTES);
        }
    }

    #[cold]
    fn find_slow(&mut self) -> Option<SuperBlock> {
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

    fn find(&mut self) -> Option<SuperBlock> {
        if let Some(block) = self.cache {
            if !block.is_full() {
                return Some(block);
            }
        }
        self.find_slow()
    }

    fn pop(&mut self) -> Option<SuperBlock> {
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
                self.dec_total_bytes(SuperBlock::BYTES);
                return Some(block);
            }
        }
        None
    }

    fn remove(&mut self, block: SuperBlock, update_stats: bool) {
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
            self.dec_total_bytes(SuperBlock::BYTES);
        }
    }

    #[cold]
    fn move_to_front_slow(&mut self, mut block: SuperBlock, alloc: bool) {
        if Some(block) == self.cache {
            return;
        }
        let group = Self::group(block, alloc);
        let block_group = block.group as usize;
        if Some(block) == self.groups[group] || group == block_group {
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

    fn move_to_front(&mut self, block: SuperBlock, alloc: bool) {
        if Some(block) == self.cache {
            return;
        }
        self.move_to_front_slow(block, alloc)
    }

    fn pop_mostly_empty_block(&mut self) -> Option<SuperBlock> {
        for i in 0..Self::GROUPS / 2 {
            if let Some(block) = self.groups[i] {
                self.groups[i] = block.next;
                if let Some(mut next) = block.next {
                    next.prev = None;
                }
                self.dec_used_bytes(block.used_bytes());
                self.dec_total_bytes(SuperBlock::BYTES);
                return Some(block);
            }
        }
        None
    }

    fn inc_used_bytes(&self, used_bytes: usize) {
        self.used_bytes.store(
            self.used_bytes.load(Ordering::Relaxed) + used_bytes,
            Ordering::Relaxed,
        )
    }

    fn dec_used_bytes(&self, used_bytes: usize) {
        self.used_bytes.store(
            self.used_bytes.load(Ordering::Relaxed) - used_bytes,
            Ordering::Relaxed,
        )
    }

    fn inc_total_bytes(&self, total_bytes: usize) {
        self.total_bytes.store(
            self.total_bytes.load(Ordering::Relaxed) + total_bytes,
            Ordering::Relaxed,
        )
    }

    fn dec_total_bytes(&self, total_bytes: usize) {
        self.total_bytes.store(
            self.total_bytes.load(Ordering::Relaxed) - total_bytes,
            Ordering::Relaxed,
        )
    }
}

pub struct Pool {
    pub global: bool,
    blocks: [Mutex<BlockList>; Self::MAX_BINS],
}

impl Pool {
    const MAX_BINS: usize = 32;

    pub const fn new(global: bool) -> Self {
        const fn create_block_list(_: usize) -> Mutex<BlockList> {
            Mutex::new(BlockList::new())
        }
        Self {
            global,
            blocks: array_const_fn_init![create_block_list; 32],
        }
    }

    pub const fn static_ref(&self) -> &'static Self {
        unsafe { &*(self as *const Self) }
    }

    pub fn push(&self, size_class: SizeClass, mut block: SuperBlock) {
        debug_assert!(!block.is_full());
        let mut blocks = self.lock_blocks(size_class);
        block.owner = self.static_ref();
        blocks.push(block, false, true);
    }

    pub fn pop(&self, size_class: SizeClass) -> Option<(SuperBlock, MutexGuard<BlockList>)> {
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
    ) -> SuperBlock {
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

    pub fn lock_blocks(&self, size_class: SizeClass) -> MutexGuard<BlockList> {
        self.blocks[size_class.as_usize()].lock()
    }

    #[cold]
    pub fn alloc_cell(
        &mut self,
        size_class: SizeClass,
        space: &Lazy<&'static HoardSpace, Local>,
    ) -> Option<Address> {
        debug_assert!(!self.global);
        let mut blocks = unsafe { self.blocks.get_unchecked(size_class.as_usize()).lock() };
        let block = if let Some(block) = blocks.find() {
            blocks.move_to_front(block, true);
            block
        } else {
            self.acquire_block_slow(size_class, &mut blocks, space)
        };
        let cell = unsafe { block.alloc_cell().unwrap_unchecked() };
        blocks.inc_used_bytes(size_class.bytes());
        Some(cell)
    }

    #[cold]
    pub fn free_cell(&self, cell: Address, space: &Lazy<&'static HoardSpace, Local>) {
        let block = SuperBlock::containing(cell);
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

    fn free_cell_slow_impl(
        &self,
        cell: Address,
        space: &Lazy<&'static HoardSpace, Local>,
        blocks: &mut BlockList,
    ) {
        let block = SuperBlock::containing(cell);
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
