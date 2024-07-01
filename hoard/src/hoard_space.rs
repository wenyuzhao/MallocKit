use super::{page_resource::BlockPageResource, Allocator, Space, SpaceId};
use crate::{pool::Pool, super_block::SuperBlock};
use mallockit::{
    space::{
        meta::{Box, Meta},
        page_resource::MemRegion,
    },
    util::{mem::alloc::discrete_tlab::DiscreteTLAB, *},
};

/// Global heap
pub struct HoardSpace {
    id: SpaceId,
    pr: BlockPageResource<SuperBlock>,
    pub(crate) pool: Pool,
}

impl Space for HoardSpace {
    const MAX_ALLOCATION_SIZE: usize = SuperBlock::BYTES / 4;
    type PR = BlockPageResource<SuperBlock>;

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
        let block = SuperBlock::containing(ptr);
        block.size_class.layout()
    }
}

impl HoardSpace {
    pub fn can_allocate(layout: Layout) -> bool {
        let layout = unsafe { layout.pad_to_align_unchecked() };
        let size = layout.size().next_power_of_two();
        size <= Self::MAX_ALLOCATION_SIZE
    }

    pub fn acquire_block(&self, size_class: SizeClass, local: &Pool) -> Option<SuperBlock> {
        // Try allocate from the global pool
        if let Some((mut block, _guard)) = self.pool.pop_most_empty_block(size_class) {
            debug_assert!(!block.is_full());
            block.owner = local.static_ref();
            debug_assert!(block.is_owned_by(local));
            return Some(block);
        }
        // Acquire new memory
        let mut block = self.pr.acquire_block()?;
        block.init(local.static_ref(), size_class);
        debug_assert!(!block.is_full());
        debug_assert!(block.is_empty());
        block.owner = local.static_ref();
        debug_assert!(block.is_owned_by(local));
        Some(block)
    }

    pub fn flush_block(&self, size_class: SizeClass, block: SuperBlock) {
        // debug_assert!(!block.is_full());
        self.pool.put(size_class, block);
    }

    pub fn release_block(&self, block: SuperBlock) {
        self.pr.release_block(block)
    }
}
/// Thread-local heap
pub struct HoardAllocator {
    tlab: DiscreteTLAB<{ SizeClass::<4>::from_bytes(Self::LARGEST_SMALL_OBJECT).as_usize() + 1 }>,
    local: Box<Pool>,
    space: &'static HoardSpace,
}

impl HoardAllocator {
    const LOCAL_HEAP_THRESHOLD: usize = 16 * 1024 * 1024;
    const LARGEST_SMALL_OBJECT: usize = 1024;

    pub fn new(space: &'static HoardSpace, _space_id: SpaceId) -> Self {
        Self {
            tlab: DiscreteTLAB::new(),
            local: Box::new_in(Pool::new(false), Meta),
            space,
        }
    }
}

impl Drop for HoardAllocator {
    fn drop(&mut self) {
        self.tlab
            .clear(|cell| self.local.free_cell(cell, self.space));
    }
}

impl Allocator for HoardAllocator {
    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let size_class = SizeClass::from_layout(layout);
        if layout.size() <= Self::LARGEST_SMALL_OBJECT {
            if let Some(cell) = self.tlab.pop(size_class) {
                return Some(cell);
            }
        }
        self.local.alloc_cell(size_class, self.space)
    }

    #[inline(always)]
    fn dealloc(&mut self, cell: Address) {
        let block = SuperBlock::containing(cell);
        let size = block.size_class.bytes();
        if size <= Self::LARGEST_SMALL_OBJECT
            && size + self.tlab.free_bytes() <= Self::LOCAL_HEAP_THRESHOLD
        // && block.is_owned_by(&self.local)
        {
            self.tlab.push(block.size_class, cell);
        } else {
            self.local.free_cell(cell, self.space);
        }
    }
}
