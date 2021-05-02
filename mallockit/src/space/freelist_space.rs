use std::ops::Range;
use crate::util::*;
use crate::util::freelist::{PointerFreeList, AbstractFreeList, AddressSpaceConfig};
use super::{Allocator, Space, SpaceId, page_resource::PageResource};

const ALLOC_ALIGNED_CELL: bool = true;

struct AddressSpace;

impl AddressSpaceConfig for AddressSpace {
    const LOG_MIN_ALIGNMENT: usize = 3;
    const LOG_COVERAGE: usize = SpaceId::LOG_MAX_SPACE_SIZE;
    const LOG_MAX_CELL_SIZE: usize = Size2M::LOG_BYTES;
}

pub struct FreeListSpace {
    id: SpaceId,
    pr: PageResource,
}

impl Space for FreeListSpace {
    const MAX_ALLOCATION_SIZE: usize = Size2M::BYTES;

    fn new(id: SpaceId) -> Self {
        Self {
            id,
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
    freelist: PointerFreeList<AddressSpace>,
}

impl FreeListAllocator {
    pub const fn new(space: Lazy<&'static FreeListSpace, Local>, space_id: SpaceId) -> Self {
        Self {
            space,
            freelist: PointerFreeList::new(space_id.address_space().start),
        }
    }

    #[cold]
    fn alloc_cell_slow(&mut self, bytes: usize) -> Option<Range<Address>> {
        let page = self.space.acquire::<Size2M>(1)?.start.start();
        self.freelist.release_aligned_cell(page, Size2M::BYTES);
        self.alloc_cell(bytes)
    }

    #[inline(always)]
    fn alloc_cell(&mut self, bytes: usize) -> Option<Range<Address>> {
        if ALLOC_ALIGNED_CELL {
            let bytes = 1 << PointerFreeList::<AddressSpace>::size_class(bytes);
            if let Some(range) = self.freelist.allocate_aligned_cell(bytes) {
                return Some(range)
            }
        } else {
            if let Some(range) = self.freelist.allocate_cell(bytes) {
                return Some(range)
            }
        }
        self.alloc_cell_slow(bytes)
    }

    #[inline(always)]
    fn dealloc_cell(&mut self, ptr: Address, bytes: usize) {
        if ALLOC_ALIGNED_CELL {
            self.freelist.release_aligned_cell(ptr, bytes);
        } else {
            self.freelist.release_cell(ptr, bytes);
        }
    }
}

impl Allocator for FreeListAllocator {
    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        let bytes = Cell::from(ptr).size();
        debug_assert_ne!(bytes, 0);
        unsafe { Layout::from_size_align_unchecked(bytes, bytes.next_power_of_two()) }
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let (extended_layout, offset) = Layout::new::<Cell>().extend(layout).unwrap();
        let Range { start, end } = self.alloc_cell(extended_layout.size())?;
        let data_start = start + offset;
        Cell::from(data_start).set(start, end - start);
        debug_assert_eq!(usize::from(data_start) & (layout.align() - 1), 0);
        Some(data_start)
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        let cell = Cell::from(ptr);
        let bytes = cell.size();
        self.dealloc_cell(cell.start(), bytes);
    }
}
