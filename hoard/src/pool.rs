use crate::{hoard_space::HoardSpace, super_block::SuperBlock};
use mallockit::{
    space::page_resource::MemRegion,
    util::{mem::size_class::SizeClass, Address},
    Plan,
};
use spin::{relax::Yield, MutexGuard};

type Mutex<T> = spin::mutex::Mutex<T, Yield>;

struct EmptyClass {
    // 0 => emoty blocks
    // classes+1 => full blocks
    groups: [Option<SuperBlock>; Self::GROUPS],
}

impl EmptyClass {
    const EMPTINESS_CLASSES: usize = 8;
    const GROUPS: usize = Self::EMPTINESS_CLASSES + 2;

    const fn new() -> Self {
        Self {
            groups: [None; Self::GROUPS],
        }
    }

    fn group(block: SuperBlock) -> usize {
        let t =
            SuperBlock::DATA_BYTES >> block.size_class.log_bytes() << block.size_class.log_bytes();
        let u = block.used_bytes();
        if u == 0 {
            0
        } else {
            1 + (Self::EMPTINESS_CLASSES * u / t)
        }
    }

    #[cold]
    fn transfer(&mut self, mut block: SuperBlock, oldg: usize, newg: usize) {
        if Some(block) == self.groups[newg] || newg == oldg {
            return;
        }
        if self.groups[oldg] == Some(block) {
            self.groups[oldg] = block.next;
        }
        if let Some(mut prev) = block.prev {
            prev.next = block.next;
        }
        if let Some(mut next) = block.next {
            next.prev = block.prev;
        }
        block.group = newg as _;
        block.next = self.groups[newg];
        block.prev = None;
        if let Some(mut head) = self.groups[newg] {
            head.prev = Some(block)
        }
        self.groups[newg] = Some(block);
    }

    fn put(&mut self, mut block: SuperBlock) {
        let group = Self::group(block);
        block.group = group as _;
        block.next = self.groups[group];
        block.prev = None;
        if let Some(mut head) = self.groups[group] {
            head.prev = Some(block)
        }
        self.groups[group] = Some(block);
        debug_assert_ne!(block.prev, Some(block));
    }

    fn remove(&mut self, block: SuperBlock) {
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

    fn pop(&mut self, group: usize) -> Option<SuperBlock> {
        if let Some(block) = self.groups[group] {
            self.groups[group] = block.next;
            if let Some(mut next) = block.next {
                next.prev = None;
            }
            return Some(block);
        }
        None
    }

    fn pop_most_empty_block(&mut self) -> Option<SuperBlock> {
        for i in 0..Self::EMPTINESS_CLASSES + 1 {
            while let Some(block) = self.groups[i] {
                // remove
                self.groups[i] = block.next;
                if let Some(mut next) = block.next {
                    next.prev = None;
                }
                let bg = Self::group(block);
                if bg > i {
                    self.put(block)
                } else {
                    return Some(block);
                }
            }
        }
        None
    }

    #[cold]
    fn free_cell(&mut self, a: Address, mut b: SuperBlock) {
        let oldg = Self::group(b);
        b.free_cell(a);
        let newg = Self::group(b);
        if oldg != newg {
            self.transfer(b, oldg, newg)
        }
    }
}

pub struct BlockList {
    cache: Option<SuperBlock>,
    groups: EmptyClass,
    used_bytes: usize,
    total_bytes: usize,
}

impl BlockList {
    const fn new() -> Self {
        Self {
            cache: None,
            groups: EmptyClass::new(),
            used_bytes: 0,
            total_bytes: 0,
        }
    }

    const fn should_flush(&self, log_obj_size: usize) -> bool {
        let u = self.used_bytes;
        let a = self.total_bytes;
        (EmptyClass::EMPTINESS_CLASSES * u) < ((EmptyClass::EMPTINESS_CLASSES - 1) * a)
            && u + ((2 * SuperBlock::BYTES) >> log_obj_size) < a
    }

    fn remove(&mut self, block: SuperBlock) {
        self.dec_used_bytes(block.used_bytes());
        self.dec_total_bytes(SuperBlock::DATA_BYTES);
        if self.cache == Some(block) {
            self.cache = None;
            return;
        }
        self.groups.remove(block);
    }

    fn pop_most_empty_block(&mut self) -> Option<SuperBlock> {
        if let Some(cache) = self.cache.take() {
            self.dec_total_bytes(SuperBlock::DATA_BYTES);
            self.dec_used_bytes(cache.used_bytes());
            return Some(cache);
        }
        let b = self.groups.pop_most_empty_block()?;
        self.dec_total_bytes(SuperBlock::DATA_BYTES);
        self.dec_used_bytes(b.used_bytes());
        Some(b)
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

    fn put(&mut self, b: SuperBlock) {
        if Some(b) == self.cache {
            return;
        }
        if let Some(c) = self.cache {
            self.groups.put(c);
        }
        self.cache = Some(b);
        self.inc_total_bytes(SuperBlock::DATA_BYTES);
        self.inc_used_bytes(b.used_bytes());
    }

    #[cold]
    fn alloc_cell_slow(&mut self, size_class: SizeClass) -> Option<Address> {
        loop {
            if self.cache.is_none() {
                self.cache = Some(self.groups.pop_most_empty_block()?);
            }
            let mut b = self.cache.unwrap();
            if let Some(a) = b.alloc_cell() {
                self.inc_used_bytes(size_class.bytes());
                return Some(a);
            } else {
                self.cache = None;
                self.groups.put(b);
            }
        }
    }

    #[inline]
    fn alloc_cell(&mut self, size_class: SizeClass) -> Option<Address> {
        if let Some(mut b) = self.cache {
            if let Some(a) = b.alloc_cell() {
                self.inc_used_bytes(size_class.bytes());
                return Some(a);
            }
        }
        self.alloc_cell_slow(size_class)
    }

    #[inline]
    fn free_cell(&mut self, a: Address, mut b: SuperBlock, size_class: SizeClass) {
        if Some(b) == self.cache {
            b.free_cell(a)
        } else {
            self.groups.free_cell(a, b);
        }
        self.dec_used_bytes(size_class.bytes());
    }
}

pub struct Pool {
    pub global: bool,
    // This is a major difference to the original hoard: we lock bins instead of the entire local heap.
    blocks: [Mutex<BlockList>; Self::MAX_BINS],
}

impl Drop for Pool {
    fn drop(&mut self) {
        let space = &crate::Hoard::get().hoard_space;
        for (i, block) in self.blocks.iter().enumerate() {
            let sz: SizeClass = SizeClass(i as _);
            let mut block = block.lock();
            if let Some(b) = block.cache.take() {
                space.flush_block(sz, b);
            }
            for i in 0..EmptyClass::GROUPS {
                while let Some(b) = block.groups.pop(i) {
                    space.flush_block(sz, b);
                }
            }
        }
    }
}

impl Pool {
    const MAX_BINS: usize = 32;

    pub const fn new(global: bool) -> Self {
        Self {
            global,
            blocks: [const { Mutex::new(BlockList::new()) }; 32],
        }
    }

    pub const fn static_ref(&self) -> &'static Self {
        unsafe { &*(self as *const Self) }
    }

    pub fn put(&self, size_class: SizeClass, mut block: SuperBlock) {
        // debug_assert!(!block.is_full());
        let mut blocks = self.lock_blocks(size_class);
        block.owner = self.static_ref();
        blocks.put(block);
    }

    pub fn pop_most_empty_block(
        &self,
        size_class: SizeClass,
    ) -> Option<(SuperBlock, MutexGuard<BlockList>)> {
        debug_assert!(self.global);
        let mut blocks = self.lock_blocks(size_class);
        if let Some(block) = blocks.pop_most_empty_block() {
            debug_assert!(block.is_owned_by(self));
            return Some((block, blocks));
        }
        None
    }

    fn lock_blocks(&self, size_class: SizeClass) -> MutexGuard<BlockList> {
        unsafe { self.blocks.get_unchecked(size_class.as_usize()).lock() }
    }

    pub fn alloc_cell(
        &mut self,
        size_class: SizeClass,
        space: &'static HoardSpace,
    ) -> Option<Address> {
        debug_assert!(!self.global);
        let mut blocks = self.lock_blocks(size_class);
        if let Some(a) = blocks.alloc_cell(size_class) {
            return Some(a);
        }
        // slow-path
        loop {
            if let Some(a) = blocks.alloc_cell(size_class) {
                return Some(a);
            }
            let block = space.acquire_block(size_class, self)?;
            blocks.put(block);
        }
    }

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
        blocks.free_cell(cell, block, block.size_class);
        if block.is_empty() {
            blocks.remove(block);
            space.release_block(block);
        }
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
        if let Some(mostly_empty_block) = blocks.pop_most_empty_block() {
            // debug_assert!(!mostly_empty_block.is_full());
            debug_assert!(mostly_empty_block.is_owned_by(self));
            space.flush_block(size_class, mostly_empty_block);
            debug_assert!(!mostly_empty_block.is_owned_by(self));
        }
    }
}
