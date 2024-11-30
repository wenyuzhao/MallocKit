use std::{
    num::NonZeroUsize,
    ops::{Deref, DerefMut, Range},
    sync::atomic::{AtomicU8, Ordering},
};

use mallockit::{
    space::page_resource::MemRegion,
    util::{constants::MIN_ALIGNMENT, mem::size_class::SizeClass},
};

use crate::{pool::Pool, ImmixAllocator};

use super::Address;

const OBJS_IN_BLOCK: usize = Block::BYTES / MIN_ALIGNMENT;

#[repr(C)]
pub struct BlockMeta {
    pub owner: usize,
    // bump_cursor: u32,
    // used_bytes: u32,
    // pub prev: Option<Block>,
    // pub next: Option<Block>,
    // pub size_class: SizeClass,
    // pub group: u8,
    // head_cell: Address,
    // pub owner: &'static Pool,
    // pub obj_size: [AtomicU8; OBJS_IN_BLOCK],
    pub line_marks: [AtomicU8; 8],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Block(NonZeroUsize);

impl MemRegion for Block {
    type Meta = BlockMeta;

    const LOG_BYTES: usize = 15;

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
    pub const LINES: usize = Self::DATA_BYTES / Line::BYTES;

    pub fn init(mut self, owner: usize) {
        self.owner = owner;
        debug_assert_eq!(Self::META_BYTES, Address::BYTES * 8);
        // self.size_class = size_class;
        // let size = size_class.bytes();
        // self.head_cell = Address::ZERO;
        // self.bump_cursor = (Address::ZERO + Self::META_BYTES).align_up(size).as_usize() as u32;
        // self.used_bytes = 0;
    }

    pub fn lines(self) -> Range<Line> {
        let start = Line::from_address(self.data_start().align_up(Line::BYTES));
        let end = Line::from_address(self.end().align_down(Line::BYTES));
        start..end
    }

    pub fn get_next_available_lines(self, search_start: Line) -> Option<Range<Line>> {
        let start_cursor = search_start.get_index_within_block();
        let mut cursor = start_cursor;
        // Find start
        while cursor < self.line_marks.len() {
            let mark = self.line_marks[cursor].load(Ordering::SeqCst);
            if mark == 0 {
                break;
            }
            cursor += 1;
        }
        if cursor == self.line_marks.len() {
            return None;
        }
        let start = Line::from_address(self.data_start() + cursor * Line::BYTES);
        // Find limit
        while cursor < self.line_marks.len() {
            let mark = self.line_marks[cursor].load(Ordering::SeqCst);
            if mark != 0 {
                break;
            }
            cursor += 1;
        }
        let end = Line::from_address(self.data_start() + cursor * Line::BYTES);
        Some(start..end)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Line(NonZeroUsize);

impl Line {
    pub fn block(self) -> Block {
        Block::containing(self.start())
    }

    pub fn get_index_within_block(self) -> usize {
        (self.start() - self.block().data_start()) / Self::BYTES
    }
}

impl MemRegion for Line {
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
