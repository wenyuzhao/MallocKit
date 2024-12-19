use std::{
    alloc::Layout,
    num::NonZeroUsize,
    ops::{Deref, DerefMut, Range},
    sync::atomic::{AtomicU8, Ordering},
};

use atomic::Atomic;
use mallockit::{
    space::page_resource::MemRegion,
    util::constants::{LOG_MIN_ALIGNMENT, MIN_ALIGNMENT},
    Mutator, Plan,
};

use super::Address;

const LOG_BYTES_IN_BLOCK: usize = 15;
const OBJS_IN_BLOCK: usize = Block::BYTES / MIN_ALIGNMENT;
const LINES_IN_BLOCK: usize = (1 << LOG_BYTES_IN_BLOCK) >> Line::LOG_BYTES;

#[derive(Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum BlockState {
    Free,
    Allocating,
    Full,
    Reusable,
}

#[repr(C)]
pub struct BlockMeta {
    pub owner: usize,
    pub obj_size: [AtomicU8; OBJS_IN_BLOCK],
    /// Num. live objects per line.
    pub line_liveness: [u8; LINES_IN_BLOCK],
    pub live_lines: usize,
    pub foreign_free: Atomic<Address>,
    pub state: BlockState,
    pub prev: Option<Block>,
    pub next: Option<Block>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block(NonZeroUsize);

impl MemRegion for Block {
    type Meta = BlockMeta;

    const LOG_BYTES: usize = LOG_BYTES_IN_BLOCK;

    fn start(&self) -> Address {
        Address::from(self.0.get())
    }

    fn from_address(addr: Address) -> Self {
        debug_assert!(!addr.is_zero());
        debug_assert!(Self::is_aligned(addr));
        Self(unsafe { NonZeroUsize::new_unchecked(usize::from(addr)) })
    }
}

impl Deref for Block {
    type Target = BlockMeta;

    fn deref(&self) -> &Self::Target {
        self.meta()
    }
}

impl DerefMut for Block {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.meta_mut() }
    }
}

impl Block {
    #[allow(unused)]
    pub const LINES: usize = Self::BYTES / Line::BYTES;
    #[allow(unused)]
    pub const DATA_LINES: usize = Self::DATA_BYTES / Line::BYTES;
    pub const REUSABLE_THRESHOLD: usize = Self::LINES / 2;

    pub fn init(&mut self, owner: usize) {
        self.owner = owner;
        self.live_lines = 0;
        self.foreign_free.store(Address::ZERO, Ordering::Relaxed);
        self.state = BlockState::Allocating;
        self.next = None;
        self.prev = None;
    }

    pub fn deinit(&mut self) {
        self.state = BlockState::Free;
        self.live_lines = 0;
        self.foreign_free.store(Address::ZERO, Ordering::Relaxed);
        self.next = None;
        self.prev = None;
    }

    pub fn lines(self) -> Range<Line> {
        let start = Line::from_address(self.data_start().align_up(Line::BYTES));
        let end = Line::from_address(self.end().align_down(Line::BYTES));
        start..end
    }

    fn remove_from_list(&mut self) {
        let owner = &mut crate::ImmixMutator::current().ix;
        if owner.reusable_blocks == Some(*self) {
            owner.reusable_blocks = self.next;
        }
        if let Some(mut prev) = self.prev {
            prev.next = self.next;
        }
        if let Some(mut next) = self.next {
            next.prev = self.prev;
        }
        self.prev = None;
        self.next = None;
    }

    pub fn get_next_available_lines(self, search_start: Line) -> Option<Range<Line>> {
        let start_cursor = search_start.get_index_within_block();
        let mut cursor = start_cursor;
        // Find start
        while cursor < self.line_liveness.len() {
            let mark = self.line_liveness[cursor];
            if mark == 0 {
                break;
            }
            cursor += 1;
        }
        let first_cursor = self.lines().start.get_index_within_block();
        if cursor != start_cursor && cursor != first_cursor {
            cursor += 1;
        }
        if cursor >= self.line_liveness.len() {
            return None;
        }
        let start = Line::from_address(self.start() + cursor * Line::BYTES);
        // Find limit
        while cursor < self.line_liveness.len() {
            let mark = self.line_liveness[cursor];
            if mark != 0 {
                break;
            }
            cursor += 1;
        }
        let end = Line::from_address(self.start() + cursor * Line::BYTES);
        if end.start() <= start.start() {
            return None;
        }
        Some(start..end)
    }

    #[inline]
    pub fn get_layout(&self, ptr: Address) -> Layout {
        let index = (ptr - self.start()) >> LOG_MIN_ALIGNMENT;
        let words = self.obj_size[index].load(Ordering::Relaxed) as usize;
        let size = words << LOG_MIN_ALIGNMENT;
        // mallockit::println!("get_layout {ptr:?} {words} {size}");
        Layout::from_size_align(size, MIN_ALIGNMENT).unwrap()
    }

    #[inline]
    pub fn on_alloc(&mut self, ptr: Address, layout: Layout) {
        let block_start = self.start();
        // Record obj size
        let words = layout.size() >> LOG_MIN_ALIGNMENT;
        let index = (ptr - block_start) >> LOG_MIN_ALIGNMENT;
        self.obj_size[index].store(words as u8, Ordering::Relaxed);
        // Update liveness counters
        let is_straddle = layout.size() > Line::BYTES;
        let mut lines = 0;
        if is_straddle {
            let end_addr = ptr + layout.size();
            let start = (ptr - block_start) >> Line::LOG_BYTES;
            let limit = (end_addr - block_start) >> Line::LOG_BYTES;
            for i in start..limit {
                if self.line_liveness[i] == 0 {
                    lines += 1;
                }
                self.line_liveness[i] += 1;
            }
        } else {
            let i = (ptr - block_start) >> Line::LOG_BYTES;
            if self.line_liveness[i] == 0 {
                lines += 1;
            }
            self.line_liveness[i] += 1;
        }
        self.live_lines += lines;
        // self.drain_foreign_free();
        // println!(" - BA {:x?} live-lines {}", self, self.live_lines);
    }

    #[inline(always)]
    fn dealloc_impl(&mut self, ptr: Address, layout: Layout) {
        let block_start = self.start();
        // Update liveness counters
        let mut dead_lines: usize = 0;
        let is_straddle = layout.size() > Line::BYTES;
        if is_straddle {
            let end_addr = ptr + layout.size();
            let start = (ptr - block_start) >> Line::LOG_BYTES;
            let limit = (end_addr - block_start) >> Line::LOG_BYTES;
            for i in start..limit {
                if self.line_liveness[i] == 1 {
                    dead_lines += 1;
                }
                // if self.line_liveness[i] == 0 {
                //     println!(" - inva;id block {:?}", self.start());
                // }
                self.line_liveness[i] -= 1;
            }
        } else {
            let i = (ptr - block_start) >> Line::LOG_BYTES;
            if self.line_liveness[i] == 1 {
                dead_lines += 1;
            }
            // if self.line_liveness[i] == 0 {
            //     println!(" - inva;id block {:?}", self.start());
            // }
            self.line_liveness[i] -= 1;
        }
        self.live_lines -= dead_lines;
        // println!(
        //     " - BD {:x?} live-lines {} (dead {})",
        //     self, self.live_lines, dead_lines
        // );
        if self.live_lines == 0 && self.state != BlockState::Allocating {
            // The block is dead
            self.remove_from_list();
            let space = &crate::Immix::get().immix_space;
            space.release_block(*self);
        } else {
            // println!(" - S-{:?} dead-{}", self.state, dead_lines);
            if dead_lines >= 1 && self.state == BlockState::Full {
                let live_lines = self.live_lines;
                if live_lines <= Block::REUSABLE_THRESHOLD {
                    let owner = &mut crate::ImmixMutator::current().ix;
                    owner.add_reusable_block(*self);
                }
            }
        }
    }

    pub fn drain_foreign_free(&mut self) {
        let mut ptr;
        loop {
            ptr = self.foreign_free.load(Ordering::Relaxed);
            if ptr.is_zero() {
                break;
            }
            if self
                .foreign_free
                .compare_exchange(ptr, Address::ZERO, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
        }
        // let mut count = 0;
        while !ptr.is_zero() {
            let next = unsafe { ptr.load() };
            self.dealloc_impl(ptr, self.get_layout(ptr));
            ptr = next;
            // count += 1;
        }
        // if count > 0 {
        //     //
        //     println!(" - DF {:x?} freed {}", self, count);
        // }
    }

    #[inline]
    pub fn on_dealloc(&mut self, ptr: Address, layout: Layout) {
        // println!("FLocal {:?}", ptr..(ptr + layout.size()));
        self.dealloc_impl(ptr, layout);
    }

    #[inline]
    pub fn on_dealloc_foreign(&self, ptr: Address) {
        // println!("FF {:?}", ptr);
        loop {
            let next = self.foreign_free.load(Ordering::SeqCst);
            unsafe { ptr.store(next) };
            if self
                .foreign_free
                .compare_exchange(next, ptr, Ordering::SeqCst, Ordering::SeqCst)
                .is_ok()
            {
                break;
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line(NonZeroUsize);

impl Line {
    pub fn block(self) -> Block {
        Block::containing(self.start())
    }

    pub fn get_index_within_block(self) -> usize {
        (self.start() - self.block().start()) / Self::BYTES
    }
}

impl MemRegion for Line {
    type Meta = ();
    const LOG_BYTES: usize = 8;

    fn start(&self) -> Address {
        Address::from(self.0.get())
    }

    fn from_address(addr: Address) -> Self {
        debug_assert!(!addr.is_zero());
        debug_assert!(Self::is_aligned(addr));
        Self(unsafe { NonZeroUsize::new_unchecked(usize::from(addr)) })
    }
}
