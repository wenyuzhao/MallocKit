use std::sync::atomic::Ordering;

use super::{page_resource::BlockPageResource, Allocator, Space, SpaceId};
use crate::{
    block::{self, Block, BlockState, Line},
    pool::Pool,
};
use constants::{LOG_MIN_ALIGNMENT, MIN_ALIGNMENT};
use mallockit::{
    space::{
        meta::{Box, Meta},
        page_resource::MemRegion,
    },
    util::{mem::alloc::discrete_tlab::DiscreteTLAB, *},
};

/// Global heap
pub struct ImmixSpace {
    id: SpaceId,
    pr: BlockPageResource<Block>,
    pub(crate) pool: Pool,
}

// const SIZE_ENCODING_SHIFT: usize = 56;

impl Space for ImmixSpace {
    const MAX_ALLOCATION_SIZE: usize = (256 - 1) * MIN_ALIGNMENT;

    type PR = BlockPageResource<Block>;

    fn new(id: SpaceId) -> Self {
        Self {
            id,
            pr: BlockPageResource::new(id),
            pool: Pool::new(true),
        }
    }

    fn id(&self) -> SpaceId {
        self.id
    }

    fn page_resource(&self) -> &Self::PR {
        &self.pr
    }

    fn get_layout(ptr: Address) -> Layout {
        let block = Block::containing(ptr);
        block.get_layout(ptr)
    }
}

impl ImmixSpace {
    pub fn can_allocate(layout: Layout) -> bool {
        if layout.align() > MIN_ALIGNMENT {
            return false;
        }
        let layout = unsafe { layout.pad_to_align_unchecked() };
        let size = layout.size().next_power_of_two();
        size <= Self::MAX_ALLOCATION_SIZE
    }

    pub fn get_clean_block(&self, owner: *const ImmixAllocator) -> Option<Block> {
        let mut block = self.pr.acquire_block()?;
        block.init(owner as usize);
        Some(block)
    }

    pub fn release_block(&self, block: Block) {
        self.pr.release_block(block)
    }
}

pub struct ImmixAllocator {
    cursor: Address,
    limit: Address,
    block: Option<Block>,
    large_cursor: Address,
    large_limit: Address,
    large_block: Option<Block>,
    request_for_large: bool,
    space: &'static ImmixSpace,
    line: Option<Line>,
    reusable_blocks: Option<Block>,
}

impl ImmixAllocator {
    const LOCAL_HEAP_THRESHOLD: usize = 16 * 1024 * 1024;
    const LARGEST_SMALL_OBJECT: usize = 1024;

    pub fn new(space: &'static ImmixSpace, _space_id: SpaceId) -> Self {
        Self {
            cursor: Address::ZERO,
            limit: Address::ZERO,
            block: None,
            space,
            large_cursor: Address::ZERO,
            large_limit: Address::ZERO,
            large_block: None,
            request_for_large: false,
            line: None,
            reusable_blocks: None,
        }
    }

    pub fn add_reusable_block(&mut self, mut block: Block) {
        // println!(" - add_reusable_block {:x?}", block);
        block.state = BlockState::Reusable;
        block.next = self.reusable_blocks;
        self.reusable_blocks = Some(block);
    }

    fn acquire_reusable_block(&mut self) -> bool {
        let Some(mut b) = self.reusable_blocks else {
            return false;
        };
        // println!(" - acquire_reusable_block {:x?}", b);
        self.reusable_blocks = b.next;
        b.next = None;
        self.line = Some(b.lines().start);
        true
    }

    fn retire_block(&mut self, large: bool) {
        let block_slot = if large { self.large_block } else { self.block };
        if let Some(mut b) = block_slot {
            // println!(
            //     " - retire_block lrg={:?} {:x?} {:?}",
            //     large, b, b.line_liveness
            // );
            let live_lines = b.live_lines;
            if b.state == BlockState::Allocating {
                if live_lines < Block::DATA_LINES / 2 {
                    self.add_reusable_block(b);
                } else {
                    b.state = BlockState::Full;
                }
            }
        }
        if large {
            self.large_block = None;
        } else {
            self.block = None;
        }
    }

    fn acquire_clean_block(&mut self) -> bool {
        match self.space.get_clean_block(self) {
            Some(block) => {
                // println!("get_clean_block {block:x?}");
                if self.request_for_large {
                    self.retire_block(true);
                    self.large_cursor = block.lines().start.start();
                    self.large_limit = block.lines().end.start();
                    self.large_block = Some(block);
                } else {
                    self.retire_block(false);
                    self.cursor = block.lines().start.start();
                    self.limit = block.lines().end.start();
                    self.block = Some(block);
                }
                true
            }
            None => false,
        }
    }

    fn acquire_reusable_lines(&mut self) -> bool {
        self.retire_block(false);
        while self.line.is_some() || self.acquire_reusable_block() {
            let line = self.line.unwrap();
            let block = line.block();
            if let Some(lines) = block.get_next_available_lines(line) {
                // Find reusable lines. Update the bump allocation cursor and limit.
                // println!("R {:x?}", lines);
                self.cursor = lines.start.start();
                self.limit = lines.end.start();
                self.block = Some(block);
                let mut block = line.block();
                block.state = BlockState::Allocating;
                self.line = if lines.end == block.lines().end {
                    None
                } else {
                    Some(lines.end)
                };
                return true;
            } else {
                self.block = None;
                self.line = None;
            }
        }
        false
    }

    fn alloc_slow(&mut self, layout: Layout, large: bool) -> Option<Address> {
        let old_request_for_large = self.request_for_large;
        self.request_for_large = large;
        let success = self.acquire_clean_block();
        self.request_for_large = old_request_for_large;
        if success {
            if large {
                let result = self.large_cursor;
                let new_cursor = self.large_cursor + layout.size();
                if new_cursor > self.large_limit {
                    None
                } else {
                    self.large_cursor = new_cursor;
                    Some(result)
                }
            } else {
                let result = self.cursor;
                let new_cursor = self.cursor + layout.size();
                if new_cursor > self.limit {
                    None
                } else {
                    self.cursor = new_cursor;
                    Some(result)
                }
            }
        } else {
            None
        }
    }

    fn alloc_slow_hot(&mut self, layout: Layout) -> Option<Address> {
        if self.acquire_reusable_lines() {
            let result = self.cursor;
            let new_cursor = self.cursor + layout.size();
            if new_cursor > self.limit {
                None
            } else {
                self.cursor = new_cursor;
                Some(result)
            }
        } else {
            self.alloc_slow(layout, false)
        }
    }
    fn overflow_alloc(&mut self, layout: Layout) -> Option<Address> {
        let start = self.large_cursor;
        let end = start + layout.size();
        if end > self.large_limit {
            self.alloc_slow(layout, true)
        } else {
            self.large_cursor = end;
            Some(start)
        }
    }
}

impl Allocator for ImmixAllocator {
    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let result = self.cursor;
        let new_cursor = self.cursor + layout.size();
        let result = if new_cursor > self.limit {
            if layout.size() > Line::BYTES {
                // Size larger than a line: do large allocation
                self.overflow_alloc(layout)
            } else {
                // Size smaller than a line: fit into holes
                self.alloc_slow_hot(layout)
            }
        } else {
            self.cursor = new_cursor;
            Some(result)
        }?;
        let mut block = Block::containing(result);
        block.on_alloc(result, layout);
        return Some(result);
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        let mut block = Block::containing(ptr);
        if block.owner == self as *const ImmixAllocator as usize {
            let layout = block.get_layout(ptr);
            block.on_dealloc(ptr, layout);
        } else {
            block.on_dealloc_foreign(ptr);
        }
    }
}

impl Drop for ImmixAllocator {
    fn drop(&mut self) {
        // self.tlab
        //     .clear(|cell| self.local.free_cell(cell, self.space));
    }
}
