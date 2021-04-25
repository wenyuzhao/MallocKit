use std::ops::Range;

use crate::util::*;
use self::{page_resource::PageResource, page_table::PageRegistry};
pub(crate) mod page_table;
pub mod page_resource;
pub mod immortal_space;


pub static PAGE_REGISTRY: PageRegistry = PageRegistry::new();

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceId(u8);

impl SpaceId {
    pub const LOG_MAX_SPACE_SIZE: usize = 41;
    pub(crate) const SHIFT: usize = Self::LOG_MAX_SPACE_SIZE;
    pub(crate) const MASK: usize = 0b1111 << Self::SHIFT;

    pub const DEFAULT: Self = Self(0);

    pub const fn next(&self) -> Self {
        debug_assert!(self.0 != 0b1111);
        Self(self.0 + 1)
    }

    pub const fn from(addr: Address) -> Self {
        let id = (usize::from(addr) & Self::MASK) >> Self::SHIFT;
        debug_assert!(id != 0);
        Self(id as u8)
    }

    pub const fn contains(&self, addr: Address) -> bool {
        Self::from(addr).0 == self.0
    }
}

pub trait Space: Sized + 'static {
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
