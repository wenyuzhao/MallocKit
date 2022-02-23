use super::super::{SpaceId, PAGE_REGISTRY};
use super::PageResource;
use crate::util::freelist::page_freelist::PageFreeList;
use crate::util::memory::RawMemory;
use crate::util::*;
use spin::Mutex;
use std::iter::Step;
use std::{
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

const NUM_SIZE_CLASS: usize = SpaceId::LOG_MAX_SPACE_SIZE - Page::<Size4K>::LOG_BYTES;

pub struct FreelistPageResource {
    pub id: SpaceId,
    freelist: Mutex<PageFreeList<{ NUM_SIZE_CLASS }>>,
    reserved_bytes: AtomicUsize,
}

impl FreelistPageResource {
    pub fn new(id: SpaceId) -> Self {
        debug_assert!(id.0 < 0b0000_1111);
        let base = id.address_space().start;
        let mut freelist = PageFreeList::new(base);
        freelist.release_cell(base, 1 << (NUM_SIZE_CLASS - 1));
        Self {
            id,
            freelist: Mutex::new(freelist),
            reserved_bytes: AtomicUsize::new(0),
        }
    }

    fn map_pages<S: PageSize>(&self, start: Page<S>, pages: usize) -> bool {
        let size = pages << S::LOG_BYTES;
        match RawMemory::map(start.start(), size) {
            Ok(_) => {
                #[cfg(target_os = "linux")]
                if cfg!(feature = "transparent_huge_page") && S::LOG_BYTES != Size4K::LOG_BYTES {
                    unsafe {
                        libc::madvise(start.start().as_mut_ptr(), size, libc::MADV_HUGEPAGE);
                    }
                }
                self.reserved_bytes
                    .fetch_add(pages << S::LOG_BYTES, Ordering::SeqCst);
                true
            }
            _ => false,
        }
    }

    fn unmap_pages<S: PageSize>(&self, start: Page<S>, pages: usize) {
        RawMemory::unmap(start.start(), pages << S::LOG_BYTES);
        self.reserved_bytes
            .fetch_sub(pages << S::LOG_BYTES, Ordering::SeqCst);
    }
}

impl PageResource for FreelistPageResource {
    #[inline(always)]
    fn reserved_bytes(&self) -> usize {
        self.reserved_bytes.load(Ordering::Relaxed)
    }

    fn acquire_pages<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        let units = pages << (S::LOG_BYTES - Size4K::LOG_BYTES);
        let start = self.freelist.lock().allocate_cell(units)?.start;
        let start = Page::<S>::new(start);
        if !self.map_pages(start, pages) {
            return self.acquire_pages(pages); // Retry
        }
        let end = Step::forward(start, pages);
        PAGE_REGISTRY.insert_pages(start, pages);
        Some(start..end)
    }

    fn release_pages<S: PageSize>(&self, start: Page<S>) {
        let pages = PAGE_REGISTRY.delete_pages(start);
        self.unmap_pages(start, pages);
        self.freelist.lock().release_cell(start.start(), pages);
    }
}
