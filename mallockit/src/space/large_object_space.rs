use std::{alloc::Layout, marker::PhantomData};

use super::{
    page_resource::{FreelistPageResource, PageResource},
    Allocator, Space, SpaceId,
};
use crate::util::{Address, Lazy, Local, Page, PageSize, Size4K};

pub struct LargeObjectSpace {
    id: SpaceId,
    pr: FreelistPageResource,
}

impl Space for LargeObjectSpace {
    type PR = FreelistPageResource;

    fn new(id: SpaceId) -> Self {
        Self {
            id,
            pr: FreelistPageResource::new(id),
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

    fn get_layout(_: Address) -> Layout {
        unreachable!()
    }
}

impl LargeObjectSpace {
    #[inline(always)]
    pub fn get_layout<S: PageSize>(&self, ptr: Address) -> Layout {
        let pages = self
            .page_resource()
            .get_contiguous_pages(Page::<S>::new(ptr));
        let bytes = pages << S::LOG_BYTES;
        unsafe { Layout::from_size_align_unchecked(bytes, bytes.next_power_of_two()) }
    }
}

pub struct LargeObjectAllocator<S: PageSize = Size4K>(
    Lazy<&'static LargeObjectSpace, Local>,
    PhantomData<S>,
);

impl<S: PageSize> LargeObjectAllocator<S> {
    pub const fn new(los: Lazy<&'static LargeObjectSpace, Local>) -> Self {
        Self(los, PhantomData)
    }

    #[inline(always)]
    fn space(&self) -> &'static LargeObjectSpace {
        *self.0
    }
}

impl<S: PageSize> Allocator for LargeObjectAllocator<S> {
    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let size = layout.size();
        let pages = (size + Page::<S>::MASK) >> Page::<S>::LOG_BYTES;
        let start_page = self.space().acquire::<S>(pages)?.start;
        debug_assert!(start_page.start().is_aligned_to(layout.align()));
        Some(start_page.start())
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        self.space().release(Page::<S>::new(ptr))
    }
}
