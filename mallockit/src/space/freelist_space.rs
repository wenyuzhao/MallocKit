use super::{page_resource::PageResource, Allocator, Space, SpaceId};
use crate::util::bits::{BitField, BitFieldSlot};
use crate::util::freelist::{AddressSpaceConfig, AlignedAbstractFreeList, AlignedFreeList};
use crate::util::freelist::{UnalignedAbstractFreeList, UnalignedFreeList};
use crate::util::*;
use spin::Mutex;
use std::{ops::Range, sync::atomic::AtomicUsize};

pub trait FreeList {
    type FreeList;
    fn can_allocate(layout: Layout) -> bool;
}

pub struct BitMapFreeList;

impl FreeList for BitMapFreeList {
    type FreeList = AlignedFreeList<AddressSpace>;

    #[inline(always)]
    fn can_allocate(layout: Layout) -> bool {
        let (extended_layout, _) = Layout::new::<Cell>().extend(layout).unwrap();
        extended_layout.size() <= FreeListSpace::MAX_ALLOCATION_SIZE
    }
}

pub struct HeaderFreeList;

impl FreeList for HeaderFreeList {
    type FreeList = UnalignedFreeList<AddressSpace, { AddressSpace::NUM_SIZE_CLASS }>;

    #[inline(always)]
    fn can_allocate(layout: Layout) -> bool {
        let (extended_layout, _) = Layout::new::<Cell>().extend(layout).unwrap();
        let unaligned_size = if extended_layout.align() != 8 {
            extended_layout.size() + extended_layout.align()
        } else {
            extended_layout.size()
        };
        unaligned_size <= FreeListSpace::MAX_ALLOCATION_SIZE
    }
}

pub struct AddressSpace;

impl AddressSpaceConfig for AddressSpace {
    const LOG_MIN_ALIGNMENT: usize = 3;
    const LOG_COVERAGE: usize = SpaceId::LOG_MAX_SPACE_SIZE;
    const LOG_MAX_CELL_SIZE: usize = Size2M::LOG_BYTES;
}

pub struct FreeListSpace {
    id: SpaceId,
    pr: PageResource,
    pages: Mutex<Option<Page<Size2M>>>,
}

impl Space for FreeListSpace {
    const MAX_ALLOCATION_SIZE: usize = Size2M::BYTES;

    fn new(id: SpaceId) -> Self {
        Self {
            id,
            pr: PageResource::new(id),
            pages: Mutex::new(None),
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
    pub fn can_allocate<FL: FreeList>(layout: Layout) -> bool {
        FL::can_allocate(layout)
    }

    #[inline(always)]
    pub fn add_coalesced_page(&self, page: Page<Size2M>) {
        let mut pages = self.pages.lock();
        let head = pages.map(|p| p.start()).unwrap_or(Address::ZERO);
        unsafe { page.start().store(head) }
        *pages = Some(page);
    }

    #[inline(always)]
    pub fn get_coalesced_page(&self) -> Option<Page<Size2M>> {
        let mut pages = self.pages.lock();
        let page = (*pages)?;
        let next = unsafe { page.start().load::<Address>() };
        *pages = if next.is_zero() {
            None
        } else {
            Some(Page::new(next))
        };
        Some(page)
    }
}

#[repr(C)]
struct Cell {
    word: AtomicUsize,
}

impl Cell {
    const START_OFFSET: BitField = BitField { bits: 21, shift: 0 };
    const SIZE: BitField = BitField {
        bits: 21,
        shift: 21,
    };
    const LOG_ALIGN: BitField = BitField { bits: 8, shift: 42 };

    const fn from(ptr: Address) -> &'static mut Self {
        unsafe { &mut *ptr.as_mut_ptr::<Self>().sub(1) }
    }
    #[inline(always)]
    fn set(&mut self, start: Address, size: usize, align: usize) {
        debug_assert!(align.is_power_of_two());
        let log_align = align.trailing_zeros() as usize;
        debug_assert!(log_align <= 21);
        let start_offset = (Address::from(self as *const _) - start) as usize;
        self.word.set::<{ Self::START_OFFSET }>(start_offset);
        self.word.set::<{ Self::SIZE }>(size);
        self.word.set::<{ Self::LOG_ALIGN }>(log_align);
    }
    #[inline(always)]
    fn start(&self) -> Address {
        Address::from(self) - self.word.get::<{ Self::START_OFFSET }>()
    }
    #[inline(always)]
    fn size(&self) -> usize {
        self.word.get::<{ Self::SIZE }>()
    }
    #[inline(always)]
    fn align(&self) -> usize {
        1 << self.word.get::<{ Self::LOG_ALIGN }>()
    }
}

pub struct FreeListAllocator<FL: FreeList> {
    space: Lazy<&'static FreeListSpace, Local>,
    freelist: FL::FreeList,
}

impl FreeListAllocator<BitMapFreeList> {
    const ALLOC_ALIGNED_CELL: bool = true;

    pub const fn new(space: Lazy<&'static FreeListSpace, Local>, space_id: SpaceId) -> Self {
        Self {
            space,
            freelist: AlignedFreeList::new(space_id.address_space().start),
        }
    }

    #[cold]
    fn alloc_cell_slow(&mut self, bytes: usize) -> Option<Range<Address>> {
        let page = match self.space.get_coalesced_page() {
            Some(page) => page,
            _ => self.space.acquire::<Size2M>(1)?.start,
        };
        self.freelist
            .release_aligned_cell(page.start(), Size2M::BYTES);
        self.alloc_cell(bytes)
    }

    #[inline(always)]
    fn alloc_cell(&mut self, bytes: usize) -> Option<Range<Address>> {
        if Self::ALLOC_ALIGNED_CELL {
            let bytes = 1 << AlignedFreeList::<AddressSpace>::size_class(bytes);
            if let Some(range) = self.freelist.allocate_aligned_cell(bytes) {
                return Some(range);
            }
        } else {
            if let Some(range) = self.freelist.allocate_cell(bytes) {
                return Some(range);
            }
        }
        self.alloc_cell_slow(bytes)
    }

    #[inline(always)]
    fn dealloc_cell(&mut self, ptr: Address, bytes: usize) {
        if Self::ALLOC_ALIGNED_CELL {
            self.freelist.release_aligned_cell(ptr, bytes);
        } else {
            self.freelist.release_cell(ptr, bytes);
        }
    }

    #[inline(always)]
    fn get_coalesced_pages(&mut self) -> Option<Page<Size2M>> {
        Some(Page::new(self.freelist.pop_raw_cell(Size2M::LOG_BYTES)?))
    }
}

impl Allocator for FreeListAllocator<BitMapFreeList> {
    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        let cell = Cell::from(ptr);
        let size = cell.size();
        let align = cell.align();
        debug_assert_ne!(size, 0);
        unsafe { Layout::from_size_align_unchecked(size, align) }
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let (extended_layout, offset) = Layout::new::<Cell>().extend(layout).unwrap();
        let Range { start, end } = self.alloc_cell(extended_layout.size())?;
        let data_start = start + offset;
        Cell::from(data_start).set(start, end - start, layout.align());
        debug_assert_eq!(usize::from(data_start) & (layout.align() - 1), 0);
        Some(data_start)
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        let cell = Cell::from(ptr);
        let bytes = cell.size();
        self.dealloc_cell(cell.start(), bytes);
        while let Some(page) = self.get_coalesced_pages() {
            self.space.add_coalesced_page(page)
        }
    }
}

impl FreeListAllocator<HeaderFreeList> {
    pub const fn new(space: Lazy<&'static FreeListSpace, Local>, space_id: SpaceId) -> Self {
        Self {
            space,
            freelist: UnalignedFreeList::new(false, space_id.address_space().start),
        }
    }

    #[cold]
    fn alloc_cell_slow(&mut self, bytes: usize) -> Option<Range<Address>> {
        let page = self.space.acquire::<Size2M>(1)?.start.start();
        self.freelist.add_units(page, Size2M::BYTES);
        self.alloc_cell(bytes)
    }

    #[inline(always)]
    fn alloc_cell(&mut self, bytes: usize) -> Option<Range<Address>> {
        if let Some(range) = self.freelist.allocate_cell(bytes) {
            return Some(range);
        }
        self.alloc_cell_slow(bytes)
    }

    #[inline(always)]
    fn dealloc_cell(&mut self, ptr: Address, bytes: usize) {
        self.freelist.release_cell(ptr, bytes);
    }

    #[inline(always)]
    fn get_coalesced_pages(&mut self) -> Option<Page<Size2M>> {
        Some(Page::new(self.freelist.pop_raw_cell(Size2M::LOG_BYTES)?))
    }
}

impl Allocator for FreeListAllocator<HeaderFreeList> {
    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        let bytes = Cell::from(ptr).size();
        debug_assert_ne!(bytes, 0);
        unsafe { Layout::from_size_align_unchecked(bytes, bytes.next_power_of_two()) }
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let (extended_layout, offset) = Layout::new::<Cell>().extend(layout).unwrap();
        let unaligned_size = if extended_layout.align() > 8 {
            extended_layout.size() + extended_layout.align()
        } else {
            extended_layout.size()
        };
        let align = extended_layout.align();
        let Range { start, end } = self.alloc_cell(unaligned_size)?;
        let mask = align - 1;
        let aligned_start = Address::from((*start + mask) & !mask);
        let data_start = aligned_start + offset;
        Cell::from(data_start).set(start, end - start, layout.align());
        debug_assert_eq!(usize::from(data_start) & (layout.align() - 1), 0);
        Some(data_start)
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        let cell = Cell::from(ptr);
        let bytes = cell.size();
        self.dealloc_cell(cell.start(), bytes);
        while let Some(page) = self.get_coalesced_pages() {
            self.space.add_coalesced_page(page)
        }
    }
}
