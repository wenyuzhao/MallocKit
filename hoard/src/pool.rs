use crate::{hoard_space::HoardSpace, super_block::SuperBlock};
use array_const_fn_init::array_const_fn_init;
use mallockit::{
    space::page_resource::MemRegion,
    util::{mem::size_class::SizeClass, Address},
};
use spin::{relax::Yield, MutexGuard};

type Mutex<T> = spin::mutex::Mutex<T, Yield>;

pub struct BlockList {
    cache: Option<SuperBlock>,
    groups: [Option<SuperBlock>; Self::GROUPS], // fullnesss groups: <25%, <50%, <75%, <100%, FULL
    used_bytes: usize,
    total_bytes: usize,
}

impl BlockList {
    const EMPTINESS_CLASSES: usize = 8;
    const GROUPS: usize = Self::EMPTINESS_CLASSES + 2;

    const fn new() -> Self {
        Self {
            cache: None,
            groups: [None; Self::GROUPS],
            used_bytes: 0,
            total_bytes: 0,
        }
    }

    const fn should_flush(&self, log_obj_size: usize) -> bool {
        let u = self.used_bytes;
        let a = self.total_bytes;
        (Self::EMPTINESS_CLASSES * u) < ((Self::EMPTINESS_CLASSES - 1) * a)
            && u + ((2 * SuperBlock::BYTES) >> log_obj_size) < a
    }

    const fn group(block: SuperBlock, alloc: bool) -> usize {
        let t =
            SuperBlock::DATA_BYTES >> block.size_class.log_bytes() << block.size_class.log_bytes();
        let u = block.used_bytes() + if alloc { block.size_class.bytes() } else { 0 };
        if u == 0 {
            return 0;
        } else {
            return 1 + (Self::EMPTINESS_CLASSES * u / t);
        }
    }

    fn push(&mut self, mut block: SuperBlock, mut alloc: bool, update_stats: bool) {
        if alloc {
            if self.cache.is_some() {
                let cache = self.cache.unwrap();
                self.cache = Some(block);
                block.group = u8::MAX;
                block = cache;
                alloc = false;
            } else {
                self.cache = Some(block);
                return;
            }
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
            self.inc_total_bytes(SuperBlock::DATA_BYTES);
        }
    }

    #[cold]
    fn find_slow(&mut self) -> Option<SuperBlock> {
        for i in 0..Self::EMPTINESS_CLASSES + 1 {
            if let Some(block) = self.groups[i] {
                debug_assert!(!block.is_full());
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
            self.dec_total_bytes(SuperBlock::DATA_BYTES);
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
        if let Some(cache) = self.cache.take() {
            return Some(cache);
        }
        for i in 0..Self::EMPTINESS_CLASSES + 1 {
            while let Some(block) = self.groups[i] {
                // remove
                self.groups[i] = block.next;
                if let Some(mut next) = block.next {
                    next.prev = None;
                }
                let bg = Self::group(block, false);
                if bg > i {
                    self.push(block, false, false)
                } else {
                    self.dec_used_bytes(block.used_bytes());
                    self.dec_total_bytes(SuperBlock::DATA_BYTES);
                    return Some(block);
                }
            }
        }
        None
    }

    const fn inc_used_bytes(&mut self, used_bytes: usize) {
        self.used_bytes += used_bytes;
    }

    const fn dec_used_bytes(&mut self, used_bytes: usize) {
        self.used_bytes -= used_bytes;
    }

    const fn inc_total_bytes(&mut self, total_bytes: usize) {
        self.total_bytes += total_bytes;
    }

    const fn dec_total_bytes(&mut self, total_bytes: usize) {
        self.total_bytes -= total_bytes;
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
        if let Some(block) = blocks.pop_mostly_empty_block() {
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
        space: &'static HoardSpace,
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
        unsafe { self.blocks.get_unchecked(size_class.as_usize()).lock() }
    }

    #[cold]
    pub fn alloc_cell(
        &mut self,
        size_class: SizeClass,
        space: &'static HoardSpace,
    ) -> Option<Address> {
        debug_assert!(!self.global);
        let mut blocks = self.lock_blocks(size_class);
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
    pub fn free_cell(&self, cell: Address, space: &'static HoardSpace) {
        let block = SuperBlock::containing(cell);
        let mut owner = block.owner;
        let mut blocks = owner.lock_blocks(block.size_class);
        while !block.is_owned_by(owner) {
            std::mem::drop(blocks);
            std::thread::yield_now();
            owner = block.owner;
            blocks = owner.lock_blocks(block.size_class);
        }
        owner.free_cell_slow_impl(cell, space, &mut blocks, block)
    }

    fn free_cell_slow_impl(
        &self,
        cell: Address,
        space: &'static HoardSpace,
        blocks: &mut BlockList,
        block: SuperBlock,
    ) {
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
        space: &'static HoardSpace,
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
