use super::{SpaceId, PAGE_REGISTRY};
use crate::util::freelist::PageFreeList;
use crate::util::*;
use freelist::AlignedAbstractFreeList;
use spin::Mutex;
use std::iter::Step;
use std::{
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

const NUM_SIZE_CLASS: usize = SpaceId::LOG_MAX_SPACE_SIZE - Page::<Size4K>::LOG_BYTES;

pub struct PageResource {
    pub id: SpaceId,
    freelist: Mutex<PageFreeList<{ NUM_SIZE_CLASS }>>,
    committed_size: AtomicUsize,
}

impl PageResource {
    pub fn new(id: SpaceId) -> Self {
        debug_assert!(id.0 < 0b0000_1111);
        let base = id.address_space().start;
        let mut freelist = PageFreeList::new(base);
        freelist.release_cell(base, 1 << (NUM_SIZE_CLASS - 1));
        Self {
            id,
            freelist: Mutex::new(freelist),
            committed_size: AtomicUsize::new(0),
        }
    }

    #[inline(always)]
    pub fn committed_size(&self) -> usize {
        self.committed_size.load(Ordering::SeqCst)
    }

    fn map_pages<S: PageSize>(&self, start: Page<S>, pages: usize) -> bool {
        let size = pages << S::LOG_BYTES;
        let addr = unsafe {
            libc::mmap(
                start.start().as_mut_ptr(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED_NOREPLACE,
                -1,
                0,
            )
        };
        if cfg!(feature = "transparent_huge_page") && S::LOG_BYTES != Size4K::LOG_BYTES {
            unsafe {
                libc::madvise(start.start().as_mut_ptr(), size, libc::MADV_HUGEPAGE);
            }
        }
        if addr == libc::MAP_FAILED {
            false
        } else {
            self.committed_size
                .fetch_add(pages << S::LOG_BYTES, Ordering::SeqCst);
            true
        }
    }

    fn unmap_pages<S: PageSize>(&self, start: Page<S>, pages: usize) {
        unsafe {
            libc::munmap(start.start().as_mut_ptr(), pages << S::LOG_BYTES);
        }
        self.committed_size
            .fetch_sub(pages << S::LOG_BYTES, Ordering::SeqCst);
    }

    pub fn acquire_pages<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
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

    pub fn release_pages<S: PageSize>(&self, start: Page<S>) {
        let pages = PAGE_REGISTRY.delete_pages(start);
        self.unmap_pages(start, pages);
        self.freelist.lock().release_cell(start.start(), pages);
    }

    pub fn get_contiguous_pages<S: PageSize>(&self, start: Page<S>) -> usize {
        PAGE_REGISTRY.get_contiguous_pages(start.start())
    }
}
