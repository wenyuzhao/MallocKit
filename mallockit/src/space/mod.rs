use std::ops::Range;
use std::ptr;
use crate::util::*;
use self::{page_resource::PageResource, page_table::PageRegistry};
pub(crate) mod page_table;
pub mod page_resource;
pub mod immortal_space;
pub mod freelist_space;
pub mod large_object_space;


pub static PAGE_REGISTRY: PageRegistry = PageRegistry::new();

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceId(u8);

impl SpaceId {
    pub(crate) const HEAP_START: Address = Address(1usize << 45);
    pub const LOG_MAX_SPACE_SIZE: usize = 41;
    pub(crate) const SHIFT: usize = Self::LOG_MAX_SPACE_SIZE;
    pub(crate) const MASK: usize = 0b1111 << Self::SHIFT;

    pub const DEFAULT: Self = Self(0);
    pub const LARGE_OBJECT_SPACE: Self = Self(1);

    pub const fn next(&self) -> Self {
        debug_assert!(self.0 != 0b1111);
        Self(self.0 + 1)
    }

    pub const fn from(addr: Address) -> Self {
        let id = (usize::from(addr) & Self::MASK) >> Self::SHIFT;
        Self(id as u8)
    }

    pub const fn contains(&self, addr: Address) -> bool {
        Self::from(addr).0 == self.0
    }

    pub const fn address_space(&self) -> Range<Address> {
        let start = Address(Self::HEAP_START.0 + ((self.0 as usize) << Self::LOG_MAX_SPACE_SIZE));
        let end = Address(start.0 + (1usize << Self::LOG_MAX_SPACE_SIZE));
        start..end
    }
}

pub trait Space: Sized + 'static {
    const MAX_ALLOCATION_SIZE: usize = usize::MAX;

    fn new(id: SpaceId) -> Self;
    fn id(&self) -> SpaceId;
    fn page_resource(&self) -> &PageResource;

    #[inline(always)]
    fn contains(&self, address: Address) -> bool {
        SpaceId::from(address) == self.id()
    }

    #[inline(always)]
    fn committed_size(&self) -> usize {
        self.page_resource().committed_size()
    }

    fn acquire<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        self.page_resource().acquire_pages(pages)
    }

    fn release<S: PageSize>(&self, start: Page<S>) {
        self.page_resource().release_pages(start)
    }
}

pub trait Allocator {
    fn get_layout(&self, ptr: Address) -> Layout;

    fn alloc(&mut self, layout: Layout) -> Option<Address>;

    #[inline(always)]
    fn alloc_zeroed(&mut self, layout: Layout) -> Option<Address> {
        let size = layout.size();
        let ptr = self.alloc(layout);
        if let Some(ptr) = ptr {
            unsafe { ptr::write_bytes(ptr.as_mut_ptr::<u8>(), 0, size) };
        }
        ptr
    }

    fn dealloc(&mut self, ptr: Address);
}