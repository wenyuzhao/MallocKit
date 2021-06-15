use self::{page_resource::PageResource, page_table::PageRegistry};
use crate::util::*;
use std::ops::Range;
use std::ptr;
pub mod freelist_space;
pub mod immortal_space;
pub mod large_object_space;
pub mod meta;
pub mod page_resource;
pub(crate) mod page_table;

pub static PAGE_REGISTRY: PageRegistry = PageRegistry::new();

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceId(u8);

impl SpaceId {
    pub(crate) const HEAP_START: Address = Address(1usize << 45);
    pub const LOG_MAX_SPACE_SIZE: usize = 41;
    pub(crate) const SHIFT: usize = Self::LOG_MAX_SPACE_SIZE;
    pub(crate) const MASK: usize = 0b1111 << Self::SHIFT;

    pub const DEFAULT: Self = Self(1);
    pub const LARGE_OBJECT_SPACE: Self = Self::DEFAULT.next();

    pub const fn next(&self) -> Self {
        debug_assert!(self.0 != 0b1111);
        let new_id = self.0 + 1;
        if new_id == 15 {
            Self(new_id + 1)
        } else {
            Self(new_id)
        }
    }

    pub const fn is_invalid(&self) -> bool {
        self.0 == 0 || self.0 == 15
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
