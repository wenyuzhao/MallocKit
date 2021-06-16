use super::{page_resource::PageResource, Allocator, Space, SpaceId};
use crate::util::*;

pub struct LargeObjectSpace {
    id: SpaceId,
    pr: PageResource,
}

impl Space for LargeObjectSpace {
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

pub struct LargeObjectAllocator(pub Lazy<&'static LargeObjectSpace, Local>);

impl LargeObjectAllocator {
    #[inline(always)]
    fn space(&self) -> &'static LargeObjectSpace {
        *self.0
    }
}

impl Allocator for LargeObjectAllocator {
    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        let pages = self
            .space()
            .page_resource()
            .get_contiguous_pages(Page::<Size4K>::new(ptr));
        let bytes = pages << Size4K::LOG_BYTES;
        unsafe { Layout::from_size_align_unchecked(bytes, bytes.next_power_of_two()) }
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let size = layout.size();
        let pages = (size + Page::<Size4K>::MASK) >> Page::<Size4K>::LOG_BYTES;
        let start_page = self.space().acquire::<Size4K>(pages)?.start;
        debug_assert_eq!(usize::from(start_page.start()) & (layout.align() - 1), 0);
        Some(start_page.start())
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        self.space().release(Page::<Size4K>::new(ptr))
    }
}
