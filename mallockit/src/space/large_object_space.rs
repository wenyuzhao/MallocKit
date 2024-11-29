use std::{alloc::Layout, marker::PhantomData};

use super::{
    meta::Meta,
    page_resource::{FreelistPageResource, PageResource},
    Allocator, Space, SpaceId,
};
use crate::util::{Address, Page, PageSize, Size4K};

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

    fn id(&self) -> SpaceId {
        self.id
    }

    fn page_resource(&self) -> &Self::PR {
        &self.pr
    }

    fn get_layout(_: Address) -> Layout {
        unreachable!()
    }
}

impl LargeObjectSpace {
    pub fn get_layout<S: PageSize>(&self, ptr: Address) -> Layout {
        let pages = self
            .page_resource()
            .get_contiguous_pages(Page::<S>::new(ptr));
        let bytes = pages << S::LOG_BYTES;
        unsafe { Layout::from_size_align_unchecked(bytes, bytes.next_power_of_two()) }
    }
}

const fn size_class<S: PageSize>(size: usize) -> usize {
    size.next_power_of_two().trailing_zeros() as usize - S::LOG_BYTES
}

pub const fn bins<S: PageSize>(max_size: usize) -> usize {
    if (max_size.next_power_of_two().trailing_zeros() as usize) < S::LOG_BYTES {
        return 0;
    }
    max_size.next_power_of_two().trailing_zeros() as usize - S::LOG_BYTES + 1
}

pub struct LargeObjectAllocator<
    S: PageSize = Size4K,
    const MAX_CACHEABLE_SIZE: usize = 0,
    const THRESHOLD_SLOP: usize = 0,
> {
    space: &'static LargeObjectSpace,
    bins: Vec<Address, Meta>,
    max_live: usize,
    live: usize,
    cleared: bool,
    _p: PhantomData<S>,
}

impl<S: PageSize, const MAX_CACHEABLE_SIZE: usize, const THRESHOLD_SLOP: usize>
    LargeObjectAllocator<S, MAX_CACHEABLE_SIZE, THRESHOLD_SLOP>
{
    const CACHE_ENABLED: bool = bins::<S>(MAX_CACHEABLE_SIZE) > 0;

    pub fn new(los: &'static LargeObjectSpace) -> Self {
        let mut bins_vec = Vec::new_in(Meta);
        bins_vec.resize(bins::<S>(MAX_CACHEABLE_SIZE), Address::ZERO);

        Self {
            space: los,
            bins: bins_vec,
            max_live: 0,
            live: 0,
            cleared: false,
            _p: PhantomData,
        }
    }

    fn space(&self) -> &'static LargeObjectSpace {
        self.space
    }

    fn alloc_slow(&mut self, layout: Layout) -> Option<Address> {
        let size = layout.size();
        let pages = (size + Page::<S>::MASK) >> Page::<S>::LOG_BYTES;
        let start_page = self.space().acquire::<S>(pages)?.start;
        debug_assert!(start_page.start().is_aligned_to(layout.align()));
        Some(start_page.start())
    }

    fn clear_bins(&mut self) {
        let space = self.space();
        for i in 0..self.bins.len() {
            let mut page = self.bins[i];
            if !page.is_zero() {
                self.bins[i] = Address::ZERO;
                while !page.is_zero() {
                    let next_page = unsafe { page.load() };
                    space.release(Page::<S>::new(page));
                    page = next_page;
                }
            }
        }
    }
}

impl<S: PageSize, const MAX_CACHEABLE_SIZE: usize, const THRESHOLD_SLOP: usize> Allocator
    for LargeObjectAllocator<S, MAX_CACHEABLE_SIZE, THRESHOLD_SLOP>
{
    #[cold]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let aligned_size = layout.size().next_power_of_two();
        if Self::CACHE_ENABLED && aligned_size <= MAX_CACHEABLE_SIZE {
            let sc = size_class::<S>(aligned_size);
            let result = if self.bins[sc].is_zero() {
                self.alloc_slow(layout)
            } else {
                let a = self.bins[sc];
                self.bins[sc] = unsafe { a.load() };
                Some(a)
            };
            if result.is_some() {
                self.live += aligned_size;
                if self.live >= self.max_live {
                    self.max_live = self.live;
                    self.cleared = false;
                }
            }
            result
        } else {
            self.alloc_slow(layout)
        }
    }

    fn dealloc(&mut self, ptr: Address) {
        let aligned_size = self.space.get_layout::<S>(ptr).size().next_power_of_two();
        if Self::CACHE_ENABLED && aligned_size <= MAX_CACHEABLE_SIZE {
            let sc = size_class::<S>(aligned_size);
            unsafe { ptr.store(self.bins[sc]) }
            self.bins[sc] = ptr;
            self.live -= usize::min(aligned_size, self.live);
            let crossed_threshold = self.max_live > self.live + (self.live >> 2);
            if THRESHOLD_SLOP != 0
                && self.live > THRESHOLD_SLOP
                && crossed_threshold
                && !self.cleared
            {
                self.clear_bins();
                self.cleared = true;
                self.max_live = self.live;
            }
        } else {
            self.space().release(Page::<S>::new(ptr))
        }
    }
}

impl<S: PageSize, const MAX_CACHEABLE_SIZE: usize, const THRESHOLD_SLOP: usize> Drop
    for LargeObjectAllocator<S, MAX_CACHEABLE_SIZE, THRESHOLD_SLOP>
{
    fn drop(&mut self) {
        if Self::CACHE_ENABLED {
            self.clear_bins();
        }
    }
}
