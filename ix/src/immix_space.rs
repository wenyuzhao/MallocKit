use super::{page_resource::BlockPageResource, Allocator, Space, SpaceId};
use crate::{
    block::{self, Block, Line},
    pool::Pool,
};
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

impl Space for ImmixSpace {
    const MAX_ALLOCATION_SIZE: usize = Block::BYTES / 2;

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
        let size = ptr 
        let block = Block::containing(ptr);
        block.size_class.layout()
    }
}

impl ImmixSpace {
    pub fn can_allocate(layout: Layout) -> bool {
        let layout = unsafe { layout.pad_to_align_unchecked() };
        let size = layout.size().next_power_of_two();
        size <= Self::MAX_ALLOCATION_SIZE
    }

    pub fn get_clean_block(&self, owner: &ImmixAllocator) -> Option<Block> {
        let block = self.pr.acquire_block()?;
        block.init(owner as *const ImmixAllocator as usize);
        Some(block)
    }

    pub fn release_block(&self, block: Block) {
        self.pr.release_block(block)
    }
}

pub struct ImmixAllocator {
    cursor: Address,
    limit: Address,
    space: &'static ImmixSpace,
    large_cursor: Address,
    large_limit: Address,
    request_for_large: bool,
    line: Option<Line>,
}

impl ImmixAllocator {
    const LOCAL_HEAP_THRESHOLD: usize = 16 * 1024 * 1024;
    const LARGEST_SMALL_OBJECT: usize = 1024;

    pub fn new(space: &'static ImmixSpace, _space_id: SpaceId) -> Self {
        Self {
            cursor: Address::ZERO,
            limit: Address::ZERO,
            space,
            large_cursor: Address::ZERO,
            large_limit: Address::ZERO,
            request_for_large: false,
            line: None,
        }
    }

    fn acquire_recyclable_block(&mut self) -> bool {
        match self.space.get_reusable_block() {
            Some(block) => {
                self.line = Some(block.start_line());
                true
            }
            _ => false,
        }
    }

    fn acquire_recyclable_block(&mut self) -> bool {
        match self.space.get_reusable_block() {
            Some(block) => {
                self.line = Some(block.start_line());
                true
            }
            _ => false,
        }
    }

    fn acquire_clean_block(&mut self) -> bool {
        match self.space.get_clean_block() {
            Some(block) => {
                if self.request_for_large {
                    self.large_cursor = block.start();
                    self.large_limit = block.end();
                } else {
                    self.cursor = block.start();
                    self.limit = block.end();
                }
                true
            }
            None => false,
        }
    }

    fn acquire_recyclable_lines(&mut self) -> bool {
        while self.line.is_some() || self.acquire_recyclable_block() {
            let line = self.line.unwrap();
            let block = line.block();
            if let Some(lines) = block.get_next_available_lines(line) {
                // Find recyclable lines. Update the bump allocation cursor and limit.
                self.cursor = lines.start.start();
                self.limit = lines.end.start();
                let block = line.block();
                self.line = if lines.end == block.lines().end {
                    None
                } else {
                    Some(lines.end)
                };
                return true;
            } else {
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
        if self.acquire_recyclable_lines() {
            let result = self.cursor;
            let new_cursor = self.cursor + layout.size();
            if new_cursor > self.limit {
                None
            } else {
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
        if new_cursor > self.limit {
            if layout.size() > Line::BYTES {
                // Size larger than a line: do large allocation
                self.overflow_alloc(layout)
            } else {
                // Size smaller than a line: fit into holes
                self.alloc_slow_hot(layout)
            }
        } else {
            Some(result)
        }
    }

    #[inline(always)]
    fn dealloc(&mut self, cell: Address) {}
}

impl Drop for ImmixAllocator {
    fn drop(&mut self) {
        // self.tlab
        //     .clear(|cell| self.local.free_cell(cell, self.space));
    }
}
