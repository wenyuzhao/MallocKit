use spin::mutex::Mutex;

use crate::util::*;

use super::{Space, SpaceId, page_resource::PageResource};



const NUM_SIZE_CLASS: usize = Size2M::LOG_BYTES + 1;

pub struct FreeListSpace {
    id: SpaceId,
    base: Address,
    pr: PageResource,
    freelist: Mutex<FreeList<{NUM_SIZE_CLASS}>>,
}

impl Space for FreeListSpace {
    const MAX_ALLOCATION_SIZE: usize = Size2M::BYTES;

    fn new(id: SpaceId) -> Self {
        Self {
            id,
            base: id.address_space().start,
            pr: PageResource::new(id),
            freelist: Mutex::new(FreeList::new()),
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


impl FreeListSpace {
    const fn address_to_unit(&self, addr: Address) -> usize {
        addr - self.base
    }

    #[inline(always)]
    pub const fn size_class(size: usize) -> usize {
        debug_assert!(size <= Size2M::BYTES);
        FreeList::<{NUM_SIZE_CLASS}>::size_class(size)
    }

    #[inline(always)]
    pub fn alloc(&self, size_class: usize) -> Option<Address> {
        if let Some(start) = self.freelist.lock().allocate(size_class).map(|x| x.start) {
            return Some(self.base + start)
        }
        let unit = self.acquire::<Size2M>(1)?.start.start() - self.base;
        self.freelist.lock().release(unit, Size2M::LOG_BYTES);
        self.alloc(size_class)
    }

    #[inline(always)]
    pub fn dealloc(&self, ptr: Address, size_class: usize) {
        let unit = self.address_to_unit(ptr);
        self.freelist.lock().release(unit, size_class);
    }
}