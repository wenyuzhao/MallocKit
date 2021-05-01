use std::intrinsics::unlikely;
use crate::util::*;
use crate::util::freelist::{PointerFreeList, AbstractFreeList, AddressSpaceConfig};
use super::{Allocator, Space, SpaceId, page_resource::PageResource};



struct AddressSpace;

impl AddressSpaceConfig for AddressSpace {
    const LOG_MIN_ALIGNMENT: usize = 3;
    const LOG_COVERAGE: usize = SpaceId::LOG_MAX_SPACE_SIZE;
    const LOG_MAX_CELL_SIZE: usize = Size2M::LOG_BYTES;
}

pub struct FreeListSpace {
    id: SpaceId,
    base: Address,
    pr: PageResource,
}

impl Space for FreeListSpace {
    const MAX_ALLOCATION_SIZE: usize = Size2M::BYTES;

    fn new(id: SpaceId) -> Self {
        Self {
            id,
            base: id.address_space().start,
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


impl FreeListSpace {
    #[inline(always)]
    pub fn size_class(size: usize) -> usize {
        debug_assert!(size <= Size2M::BYTES);
        PointerFreeList::<AddressSpace>::size_class(size)
    }

    pub fn can_allocate(layout: Layout) -> bool {
        let (extended_layout, _) = Layout::new::<Cell>().extend(layout).unwrap();
        extended_layout.size() < FreeListSpace::MAX_ALLOCATION_SIZE
    }
}



#[repr(C)]
struct Cell(Address, usize);

impl Cell {
    const fn from(ptr: Address) -> &'static mut Self {
        unsafe { &mut *ptr.as_mut_ptr::<Self>().sub(1) }
    }
    const fn set(&mut self, start: Address, size: usize) {
        self.0 = start;
        self.1 = size;
    }
    const fn size(&self) -> usize {
        self.1
    }
    const fn start(&self) -> Address {
        self.0
    }
}

pub struct FreeListAllocator {
    space: Lazy<&'static FreeListSpace, Local>,
    base: Address,
    freelist: PointerFreeList<AddressSpace>,
}

impl FreeListAllocator {
    pub const fn new(space: Lazy<&'static FreeListSpace, Local>, space_id: SpaceId) -> Self {
        Self {
            space,
            base: Address::ZERO,
            freelist: PointerFreeList::new(space_id.address_space().start),
        }
    }

    #[inline(always)]
    fn alloc_cell(&mut self, size_class: usize) -> Option<Address> {
        if unlikely(self.base.is_zero()) {
            self.base = self.space.base;
        }
        if let Some(start) = self.freelist.allocate_cell(1 << size_class).map(|x| x.start) {
            return Some(start)
        }
        let page = self.space.acquire::<Size2M>(1)?.start.start();
        self.freelist.release_cell(page, Size2M::BYTES);
        self.alloc_cell(size_class)
    }

    #[inline(always)]
    fn dealloc_cell(&mut self, ptr: Address, size_class: usize) {
        self.freelist.release_cell(ptr, 1 << size_class);
    }
}

impl Allocator for FreeListAllocator {
    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        let bytes = Cell::from(ptr).size();
        debug_assert_ne!(bytes, 0);
        debug_assert!(bytes.is_power_of_two(), "{:?}", bytes);
        unsafe { Layout::from_size_align_unchecked(bytes, bytes) }
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let (extended_layout, offset) = Layout::new::<Cell>().extend(layout).unwrap();
        let size_class = FreeListSpace::size_class(extended_layout.size());
        let start = self.alloc_cell(size_class)?;
        let data_start = start + offset;
        Cell::from(data_start).set(start, 1 << size_class);
        debug_assert_eq!(usize::from(data_start) & (layout.align() - 1), 0);
        Some(data_start)
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        let cell = Cell::from(ptr);
        let bytes = cell.size();
        debug_assert!(bytes.is_power_of_two(), "{:x?} {:?} {:?}", bytes, cell as *mut _, ptr);
        let size_class = FreeListSpace::size_class(bytes);
        self.dealloc_cell(cell.start(), size_class);
    }
}
