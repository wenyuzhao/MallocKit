use super::{page_resource::PageResource, Allocator, Space, SpaceId};
use crate::util::*;

pub struct ImmortalSpace {
    id: SpaceId,
    pr: PageResource,
}

impl Space for ImmortalSpace {
    fn new(id: SpaceId) -> Self {
        Self {
            id,
            pr: PageResource::new(id),
        }
    }

    #[inline(always)]
    fn id(&self) -> SpaceId {
        self.id
    }

    #[inline(always)]
    fn page_resource(&self) -> &PageResource {
        &self.pr
    }
}

pub struct BumpAllocator {
    space: Lazy<&'static ImmortalSpace, Local>,
    allocation_area: AllocationArea,
    retry: bool,
}

impl BumpAllocator {
    pub const fn new(space: Lazy<&'static ImmortalSpace, Local>) -> Self {
        Self {
            space,
            allocation_area: AllocationArea::EMPTY,
            retry: false,
        }
    }

    #[cold]
    fn alloc_slow(&mut self, layout: Layout) -> Option<Address> {
        assert!(!self.retry);
        let block_size = Size2M::BYTES;
        let alloc_size = AllocationArea::align_up(
            usize::max(layout.size(), block_size) + std::mem::size_of::<Layout>(),
            Size2M::BYTES,
        );
        let alloc_pages = alloc_size >> Size2M::LOG_BYTES;
        let pages = self.space.acquire::<Size2M>(alloc_pages)?;
        let top = pages.start.start();
        let limit = pages.end.start();
        self.allocation_area = AllocationArea { top, limit };
        self.retry = true;
        let result = self.alloc(layout);
        self.retry = false;
        result
    }
}

impl Allocator for BumpAllocator {
    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        AllocationArea::load_layout(ptr)
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        if let Some(ptr) = self.allocation_area.alloc_with_layout(layout) {
            return Some(ptr);
        }
        self.alloc_slow(layout)
    }

    #[inline(always)]
    fn dealloc(&mut self, _: Address) {}
}
