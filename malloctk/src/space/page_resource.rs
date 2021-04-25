use std::{ops::Range, sync::atomic::{AtomicUsize, Ordering}};
use std::iter::Step;
use crate::util::*;
use spin::Mutex;
use super::{PAGE_REGISTRY, SpaceId};

#[derive(Debug)]
struct Cell {
    next: Option<Box<Cell, System>>,
    unit: usize,
}

const NUM_SIZE_CLASS: usize = SpaceId::LOG_MAX_SPACE_SIZE - Page::<Size4K>::LOG_BYTES + 1;
// const LOG_PAGE_SIZE: usize = 12;

pub struct PageResource {
    pub id: SpaceId,
    base: Address,
    freelist: Mutex<FreeList<{NUM_SIZE_CLASS}>>,
    committed_size: AtomicUsize,
}

impl PageResource {
    pub fn new(id: SpaceId) -> Self {
        debug_assert!(id.0 < 0b0000_1111);
        let base = Address::from(1usize << 45) + ((id.0 as usize) << 41);
        let mut freelist = FreeList::new();
        freelist.release(0, 1 << (NUM_SIZE_CLASS - 1));
        Self {
            id,
            base,
            freelist: Mutex::new(freelist),
            committed_size: AtomicUsize::new(0),
        }
    }

    #[inline(always)]
    pub fn committed_size(&self) -> usize {
        self.committed_size.load(Ordering::SeqCst)
    }

    fn pages_to_units<S: PageSize>(pages: usize) -> usize {
        pages << (S::LOG_BYTES - Size4K::LOG_BYTES)
    }

    fn page_to_unit<S: PageSize>(&self, page: Page<S>) -> usize {
        (page.start() - self.base) >> Page::<Size4K>::LOG_BYTES
    }

    fn unit_to_page<S: PageSize>(&self, unit: usize) -> Page<S> {
        Page::<S>::new(self.base + (unit << Page::<Size4K>::LOG_BYTES))
    }

    fn map_pages<S: PageSize>(&self, start: Page<S>, pages: usize) -> bool {
        let size = pages << S::LOG_BYTES;
        let addr = unsafe { libc::mmap(start.start().as_mut_ptr(), size, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED_NOREPLACE, -1, 0) };
        if cfg!(feature="transparent_huge_page") && S::LOG_BYTES != Size4K::LOG_BYTES {
            unsafe { libc::madvise(start.start().as_mut_ptr(), size, libc::MADV_HUGEPAGE); }
        }
        if addr == libc::MAP_FAILED {
            false
        } else {
            self.committed_size.fetch_add(pages << S::LOG_BYTES, Ordering::SeqCst);
            true
        }
    }

    fn unmap_pages<S: PageSize>(&self, start: Page<S>, pages: usize) {
        unsafe { libc::munmap(start.start().as_mut_ptr(), pages << S::LOG_BYTES); }
        self.committed_size.fetch_sub(pages << S::LOG_BYTES, Ordering::SeqCst);
    }

    pub fn acquire_pages<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        let units = Self::pages_to_units::<S>(pages);
        let start_unit = self.freelist.lock().allocate(units)?.start;
        let start = self.unit_to_page(start_unit);
        if !self.map_pages(start, pages) {
            return self.acquire_pages(pages); // Retry
        }
        let end = Step::forward(start, pages);
        PAGE_REGISTRY.insert_pages(start, pages);
        Some(start..end)
    }

    pub fn release_pages<S: PageSize>(&self, start: Page<S>) {
        let pages = PAGE_REGISTRY.delete_pages(start);
        self.unmap_pages(start, pages);
        let units = Self::pages_to_units::<S>(pages);
        let start_unit = self.page_to_unit(start);
        self.freelist.lock().release(start_unit, units);
    }
}