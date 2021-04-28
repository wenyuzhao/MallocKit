use super::{Allocator, Space, SpaceId, page_resource::PageResource};
use crate::util::*;


pub struct LargeObjectSpace {
    id: SpaceId,
    pr: PageResource,
}

impl Space for LargeObjectSpace {
    fn new(id: SpaceId) -> Self {
        Self {
            id,
            pr: PageResource::new(id)
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
        let pages = self.space().page_resource().get_contiguous_pages(Page::<Size2M>::new(ptr));
        let bytes = pages << Size2M::LOG_BYTES;
        debug_assert!(bytes.is_power_of_two());
        unsafe { Layout::from_size_align_unchecked(bytes, bytes) }
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let size = layout.size();
        let pages = (size + Page::<Size2M>::MASK) >> Page::<Size2M>::LOG_BYTES;
        let start_page = self.space().acquire::<Size2M>(pages)?.start;
        debug_assert_eq!(usize::from(start_page.start()) & (layout.align() - 1), 0);
        Some(start_page.start())
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        self.space().release(Page::<Size2M>::new(ptr))
    }
}
