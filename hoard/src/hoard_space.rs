use super::{page_resource::BlockPageResource, Allocator, Space, SpaceId};
use crate::{
    pool::Pool,
    super_block::{BlockExt, SuperBlock},
};
use mallockit::util::{size_class::SizeClass, *};

/// Global heap
pub struct HoardSpace {
    id: SpaceId,
    pr: BlockPageResource,
    pub(crate) pool: Pool,
}

impl Space for HoardSpace {
    const MAX_ALLOCATION_SIZE: usize = SuperBlock::BYTES / 4;
    type PR = BlockPageResource;

    fn new(id: SpaceId) -> Self {
        Self {
            id,
            pr: BlockPageResource::new(id, SuperBlock::LOG_BYTES),
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
        let block = SuperBlock::containing(ptr);
        block.size_class.layout()
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
    pub fn acquire_block(
        &self,
        size_class: SizeClass,
        local: &Pool,
        mut register: impl FnMut(SuperBlock),
    ) -> Option<SuperBlock> {
        // Try allocate from the global pool
        if let Some((block, _guard)) = self.pool.pop(size_class) {
            debug_assert!(!block.is_full());
            register(block);
            debug_assert!(block.is_owned_by(local));
            return Some(block);
        }
        // Acquire new memory
        let addr = self
            .acquire::<Size4K>(1 << (SuperBlock::LOG_BYTES - Size4K::LOG_BYTES))?
            .start
            .start();
        let block = SuperBlock::new(addr);
        block.init(local.static_ref(), size_class);
        debug_assert!(!block.is_full());
        debug_assert!(block.is_empty());
        register(block);
        debug_assert!(block.is_owned_by(local));
        Some(block)
    }

    pub fn flush_block(&self, size_class: SizeClass, block: SuperBlock) {
        debug_assert!(!block.is_full());
        self.pool.push(size_class, block);
    }

    pub fn release_block(&self, block: SuperBlock) {
        self.release::<Size4K>(Page::new(block.start()));
    }
}
/// Thread-local heap
pub struct HoardAllocator {
    space: Lazy<&'static HoardSpace, Local>,
    local: Lazy<Box<Pool>, Local>,
}

impl HoardAllocator {
    pub const fn new(space: Lazy<&'static HoardSpace, Local>, _space_id: SpaceId) -> Self {
        Self {
            space,
            local: Lazy::new(|| Box::new(Pool::new(false))),
        }
    }
}

impl Allocator for HoardAllocator {
    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let size_class = SizeClass::from_layout(layout);
        self.local.alloc_cell(size_class, &self.space)
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        self.local.free_cell(ptr, &self.space);
    }
}
