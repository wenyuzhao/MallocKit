use super::{
    page_resource::{Block, BlockPageResource},
    Allocator, Space, SpaceId,
};
use crate::util::bits::{BitField, BitFieldSlot};
use crate::util::mem::freelist::intrusive_freelist::AddressSpaceConfig;
use crate::util::mem::freelist::intrusive_freelist::IntrusiveFreeList;
use crate::util::mem::heap::HEAP;
use crate::util::*;
use spin::Mutex;
use std::{ops::Range, sync::atomic::AtomicUsize};

// type ActivePageSize = Size4K;
type ActivePageSize = Size2M;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(C)]
pub struct Chunk(Address);

impl Block for Chunk {
    const LOG_BYTES: usize = ActivePageSize::LOG_BYTES;

    fn start(&self) -> Address {
        self.0
    }

    fn from_address(addr: Address) -> Self {
        Self(addr)
    }

    fn set_next(&self, _next: Option<Self>) {
        unreachable!()
    }

    fn next(&self) -> Option<Self> {
        unreachable!()
    }
}

pub struct AddressSpace;

impl AddressSpaceConfig for AddressSpace {
    const LOG_MIN_ALIGNMENT: usize = 3;
    const LOG_COVERAGE: usize = SpaceId::LOG_MAX_SPACE_SIZE;
    const LOG_MAX_CELL_SIZE: usize = ActivePageSize::LOG_BYTES;
}

pub struct FreeListSpace {
    id: SpaceId,
    pr: BlockPageResource<Chunk, false>,
    pages: Mutex<Option<Page<ActivePageSize>>>,
}

impl Space for FreeListSpace {
    const MAX_ALLOCATION_SIZE: usize = Size4K::BYTES;
    type PR = BlockPageResource<Chunk, false>;

    fn new(id: SpaceId) -> Self {
        Self {
            id,
            pr: BlockPageResource::new(id),
            pages: Mutex::new(None),
        }
    }

    fn id(&self) -> SpaceId {
        self.id
    }

    fn page_resource(&self) -> &Self::PR {
        &self.pr
    }

    fn get_layout(ptr: Address) -> Layout {
        let cell = Cell::from(ptr);
        let bytes = cell.data_size();
        let align = cell.align();
        debug_assert_ne!(bytes, 0);
        debug_assert_ne!(align, 0);
        unsafe { Layout::from_size_align_unchecked(bytes, align) }
    }
}

impl FreeListSpace {
    pub fn can_allocate(layout: Layout) -> bool {
        let (extended_layout, _) = unsafe { Layout::new::<Cell>().extend_unchecked(layout) };
        extended_layout.padded_size() + IntrusiveFreeList::<AddressSpace>::HEADER_SIZE
            <= FreeListSpace::MAX_ALLOCATION_SIZE
    }

    fn add_coalesced_page(&self, page: Page<ActivePageSize>) {
        let mut pages = self.pages.lock();
        let head = pages.map(|p| p.start()).unwrap_or(Address::ZERO);
        unsafe { page.start().store(head) }
        *pages = Some(page);
    }

    fn get_coalesced_page(&self) -> Option<Page<ActivePageSize>> {
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
    fn set(&mut self, start: Address, size: usize, align: usize) {
        debug_assert!(align.is_power_of_two());
        let log_align = align.trailing_zeros() as usize;
        debug_assert!(log_align <= 21);
        let start_offset = unsafe { Address::from((self as *const Self).add(1)) - start };
        self.word.set(Self::START_OFFSET, start_offset);
        self.word.set(Self::SIZE, size);
        self.word.set(Self::LOG_ALIGN, log_align);
    }
    fn start(&self) -> Address {
        Address::from(self) + std::mem::size_of::<Self>() - self.word.get(Self::START_OFFSET)
    }
    fn size(&self) -> usize {
        self.word.get(Self::SIZE)
    }
    fn data_size(&self) -> usize {
        self.size() - self.word.get(Self::START_OFFSET)
    }
    fn align(&self) -> usize {
        1 << self.word.get(Self::LOG_ALIGN)
    }
}

pub struct FreeListAllocator {
    space: Lazy<&'static FreeListSpace, Local>,
    freelist: Lazy<IntrusiveFreeList<AddressSpace>, Local>,
}

impl FreeListAllocator {
    pub const fn new<const SPACE_ID: SpaceId>(space: Lazy<&'static FreeListSpace, Local>) -> Self {
        Self {
            space,
            freelist: Lazy::new(|| {
                IntrusiveFreeList::new(false, HEAP.get_space_range(SPACE_ID).start)
            }),
        }
    }

    #[cold]
    fn alloc_cell_slow(&mut self, bytes: usize) -> Option<Range<Address>> {
        let range = match self.space.get_coalesced_page() {
            Some(page) => page.range(),
            _ => self.space.pr.acquire_block()?.data(),
        };
        self.freelist.add_units(range.start, ActivePageSize::BYTES);
        self.alloc_cell(bytes)
    }

    fn alloc_cell(&mut self, bytes: usize) -> Option<Range<Address>> {
        if let Some(range) = self.freelist.allocate_cell(bytes) {
            return Some(range);
        }
        self.alloc_cell_slow(bytes)
    }

    fn dealloc_cell(&mut self, ptr: Address, bytes: usize) {
        self.freelist.release_cell(ptr, bytes);
    }

    fn get_coalesced_pages(&mut self) -> Option<Page<ActivePageSize>> {
        Some(Page::new(
            self.freelist.pop_raw_cell(ActivePageSize::LOG_BYTES)?,
        ))
    }
}

impl Allocator for FreeListAllocator {
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let (extended_layout, offset) = unsafe { Layout::new::<Cell>().extend_unchecked(layout) };
        let Range { start, end } = self.alloc_cell(extended_layout.padded_size())?;
        let aligned_start = start.align_up(extended_layout.align());
        let data_start = aligned_start + offset;
        debug_assert!(end - data_start >= layout.size());
        Cell::from(data_start).set(start, end - start, layout.align());
        debug_assert_eq!(usize::from(data_start) & (layout.align() - 1), 0);
        Some(data_start)
    }

    fn dealloc(&mut self, ptr: Address) {
        let cell = Cell::from(ptr);
        let bytes = cell.size();
        self.dealloc_cell(cell.start(), bytes);
        while let Some(page) = self.get_coalesced_pages() {
            self.space.add_coalesced_page(page)
        }
    }
}
