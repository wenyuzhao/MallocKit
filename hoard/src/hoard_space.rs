use super::{page_resource::BlockPageResource, Allocator, Space, SpaceId};
use crate::{
    block::{Block, BlockExt},
    pool::{BlockList, Pool},
};
use mallockit::util::*;

/// Global heap
pub struct HoardSpace {
    id: SpaceId,
    pr: BlockPageResource,
    pub(crate) pool: Pool,
}

impl Space for HoardSpace {
    const MAX_ALLOCATION_SIZE: usize = Block::BYTES / 4;
    type PR = BlockPageResource;

    fn new(id: SpaceId) -> Self {
        Self {
            id,
            pr: BlockPageResource::new(id, Block::LOG_BYTES),
            pool: Pool::new(true),
        }
    }

    #[inline(always)]
    fn id(&self) -> SpaceId {
        self.id
    }

    #[inline(always)]
    fn page_resource(&self) -> &Self::PR {
        &self.pr
    }

    #[inline(always)]
    fn get_layout(ptr: Address) -> Layout {
        let block = Block::containing(ptr);
        let size = 1usize << (block.size_class + 3);
        Layout::from_size_align(size, size).unwrap()
    }
}

impl HoardSpace {
    #[inline(always)]
    pub fn can_allocate(layout: Layout) -> bool {
        let layout = unsafe { layout.pad_to_align_unchecked() };
        let size = layout.size().next_power_of_two();
        size <= Self::MAX_ALLOCATION_SIZE
    }

    #[inline(always)]
    pub fn size_class_of_layout(layout: Layout) -> usize {
        let layout = unsafe { layout.pad_to_align_unchecked() };
        let size = layout.size().next_power_of_two();
        Self::size_class(size)
    }

    #[inline(always)]
    pub const fn size_class(bytes: usize) -> usize {
        // debug_assert!(bytes <= Self::MAX_ALLOCATION_SIZE);
        bytes.trailing_zeros() as usize - 3
    }

    #[inline(always)]
    pub const fn size_class_to_bytes(size_class: usize) -> usize {
        1 << (size_class + 3)
    }

    #[inline(always)]
    pub fn acquire_block(
        &self,
        local: &Pool,
        size_class: usize,
        block_list: &mut BlockList,
    ) -> Option<Block> {
        // Try allocate from the global pool
        if let Some((mut block, _guard)) = self.pool.pop_back(size_class) {
            block_list.push_back(block);
            block.owner = Some(local.static_ref());
            debug_assert!(!block.is_full());
            return Some(block);
        }
        // Acquire new memory
        let addr = self
            .acquire::<Size4K>(1 << (Block::LOG_BYTES - Size4K::LOG_BYTES))?
            .start
            .start();
        let mut block = Block::new(addr);
        block.init(local.static_ref(), size_class);
        block_list.push_back(block);
        block.owner = Some(local.static_ref());
        debug_assert!(!block.is_full());
        Some(block)
    }

    #[inline(always)]
    pub fn flush_block(&self, size_class: usize, block: Block) {
        debug_assert!(!block.is_full());
        self.pool.push_pack(size_class, block);
    }

    #[inline(always)]
    pub fn release_block(&self, block: Block) {
        self.release::<Size4K>(Page::new(block.start()));
    }
}
/// Thread-local heap
pub struct HoardAllocator {
    space: Lazy<&'static HoardSpace, Local>,
    local: Pool,
}

impl HoardAllocator {
    pub const fn new(space: Lazy<&'static HoardSpace, Local>, _space_id: SpaceId) -> Self {
        Self {
            space,
            local: Pool::new(false),
        }
    }
}

impl Allocator for HoardAllocator {
    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let size_class = HoardSpace::size_class_of_layout(layout);
        self.local.alloc_cell(size_class, &self.space)
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        let block = Block::containing(ptr);
        block.owner.unwrap().free_cell(ptr, &self.space);
    }
}
