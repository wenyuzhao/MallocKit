use self::page_resource::PageResource;
use crate::util::*;
use std::ops::Range;
pub mod freelist_space;
pub mod immortal_space;
pub mod large_object_space;
pub mod meta;
pub mod page_resource;
pub(crate) mod page_table;
use std::marker::ConstParamTy;

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, ConstParamTy)]
pub struct SpaceId(pub(crate) u8);

impl SpaceId {
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

    pub fn from(addr: Address) -> Self {
        let id = (usize::from(addr) & Self::MASK) >> Self::SHIFT;
        Self(id as u8)
    }

    pub fn contains(&self, addr: Address) -> bool {
        Self::from(addr).0 == self.0
    }
}

pub trait Space: Sized + 'static {
    const MAX_ALLOCATION_SIZE: usize = usize::MAX;
    type PR: PageResource;

    fn new(id: SpaceId) -> Self;
    fn id(&self) -> SpaceId;
    fn page_resource(&self) -> &Self::PR;

    fn get_layout(ptr: Address) -> Layout;

    fn contains(&self, address: Address) -> bool {
        SpaceId::from(address) == self.id()
    }

    fn reserved_bytes(&self) -> usize {
        self.page_resource().reserved_bytes()
    }

    fn acquire<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        self.page_resource().acquire_pages(pages)
    }

    fn release<S: PageSize>(&self, start: Page<S>) {
        self.page_resource().release_pages(start)
    }
}

pub trait Allocator {
    fn alloc(&mut self, layout: Layout) -> Option<Address>;

    fn dealloc(&mut self, ptr: Address);

    // TODO: realloc
}
