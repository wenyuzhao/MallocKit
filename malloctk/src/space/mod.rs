use crate::util::Address;
use self::{page_resource::PageResource, page_table::PageRegistry};
pub(crate) mod page_table;
pub mod page_resource;
pub mod immortal_space;

// static SPACE_ID_COUNTER:

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
}

pub trait Space: Sized + 'static {
    fn new(id: SpaceId) -> Self;
    fn id(&self) -> SpaceId;
    fn page_resource(&self) -> &PageResource;
    fn acquire(&self, pages: usize) -> Option<Address> {
        self.page_resource().acquire_pages(pages)
    }
    fn release(&self, start: Address) {
        self.page_resource().release_pages(start)
    }
}
