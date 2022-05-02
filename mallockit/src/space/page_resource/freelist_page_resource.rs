use super::super::SpaceId;
use super::PageResource;
use crate::util::freelist::page_freelist::PageFreeList;
use crate::util::memory::RawMemory;
use crate::util::*;
use spin::mutex::Mutex;
use spin::rwlock::RwLock;
use spin::Yield;
use std::intrinsics::unlikely;
use std::iter::Step;
use std::sync::atomic::AtomicU8;
use std::{
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

const NUM_SIZE_CLASS: usize = SpaceId::LOG_MAX_SPACE_SIZE - Page::<Size4K>::LOG_BYTES;

pub struct FreelistPageResource {
    pub id: SpaceId,
    freelist: Mutex<PageFreeList<{ NUM_SIZE_CLASS }>, Yield>,
    reserved_bytes: AtomicUsize,
    meta: RwLock<Vec<AtomicU8>, Yield>,
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
            meta: RwLock::new(unsafe { std::mem::transmute(vec![0u8; 1 << 20]) }),
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

    #[inline(always)]
    fn set_meta<S: PageSize>(&self, start: Page<S>, pages: usize) {
        let index = (start.start() - self.id.address_space().start) >> Page::<Size4K>::LOG_BYTES;
        let log_pages = pages.next_power_of_two().trailing_zeros() as u8;
        let meta = self.meta.upgradeable_read();
        if unlikely(index >= meta.len()) {
            let mut meta = meta.upgrade();
            let len = meta.len();
            meta.resize_with(len << 1, AtomicU8::default);
            meta[index].store(log_pages, Ordering::Relaxed);
        } else {
            meta[index].store(log_pages, Ordering::Relaxed);
        }
    }

    #[inline(always)]
    fn get_meta<S: PageSize>(&self, start: Page<S>) -> usize {
        let index = (start.start() - self.id.address_space().start) >> Page::<Size4K>::LOG_BYTES;
        let log_pages = self.meta.read()[index].load(Ordering::Relaxed);
        1usize << log_pages
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
        self.set_meta(start, units);
        Some(start..end)
    }

    fn release_pages<S: PageSize>(&self, start: Page<S>) {
        let pages = self.get_meta(start) >> (S::LOG_BYTES - Size4K::LOG_BYTES);
        self.unmap_pages(start, pages);
        self.freelist.lock().release_cell(start.start(), pages);
    }

    fn get_contiguous_pages<S: PageSize>(&self, start: Page<S>) -> usize {
        self.get_meta(start) >> (S::LOG_BYTES - Size4K::LOG_BYTES)
    }
}
