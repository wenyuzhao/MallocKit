use super::super::SpaceId;
use super::PageResource;
use crate::util::freelist::page_freelist::PageFreeList;
use crate::util::heap::HEAP;
use crate::util::memory::RawMemory;
use crate::util::*;
use spin::mutex::Mutex;
use spin::rwlock::RwLock;
use spin::Yield;
use std::iter::Step;
use std::sync::atomic::AtomicU32;
use std::{
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

const NUM_SIZE_CLASS: usize = SpaceId::LOG_MAX_SPACE_SIZE - Page::<Size4K>::LOG_BYTES;

pub struct FreelistPageResource {
    pub id: SpaceId,
    freelist: Mutex<PageFreeList<{ NUM_SIZE_CLASS }>, Yield>,
    reserved_bytes: AtomicUsize,
    meta: RwLock<Vec<AtomicU32>, Yield>,
    base: Address,
}

impl FreelistPageResource {
    pub fn new(id: SpaceId) -> Self {
        debug_assert!(id.0 < 0b0000_1111);
        let range = HEAP.get_space_range(id);
        let base = range.start;
        let mut freelist = PageFreeList::new(base);
        freelist.release_cell(base, 1 << (NUM_SIZE_CLASS - 1));
        Self {
            id,
            freelist: Mutex::new(freelist),
            reserved_bytes: AtomicUsize::new(0),
            meta: RwLock::new(unsafe { std::mem::transmute(vec![0u32; 1 << 20]) }),
            base,
        }
    }

    fn map_pages<S: PageSize>(&self, _start: Page<S>, pages: usize) {
        self.reserved_bytes
            .fetch_add(pages << S::LOG_BYTES, Ordering::SeqCst);
    }

    fn unmap_pages<S: PageSize>(&self, start: Page<S>, pages: usize) {
        RawMemory::madv_free(start.start(), pages << S::LOG_BYTES);
        self.reserved_bytes
            .fetch_sub(pages << S::LOG_BYTES, Ordering::SeqCst);
    }

    fn set_meta<S: PageSize>(&self, start: Page<S>, pages: usize) {
        debug_assert!(pages <= u32::MAX as usize);
        let index = (start.start() - self.base) >> Page::<Size4K>::LOG_BYTES;
        let meta = self.meta.upgradeable_read();
        if index >= meta.len() {
            let mut meta = meta.upgrade();
            let len = usize::max(meta.len(), index).next_power_of_two();
            meta.resize_with(len << 1, Default::default);
            meta[index].store(pages as _, Ordering::Relaxed);
        } else {
            meta[index].store(pages as _, Ordering::Relaxed);
        }
    }

    fn get_meta<S: PageSize>(&self, start: Page<S>) -> usize {
        let index = (start.start() - self.base) >> Page::<Size4K>::LOG_BYTES;
        self.meta.read()[index].load(Ordering::Relaxed) as _
    }
}

impl PageResource for FreelistPageResource {
    fn reserved_bytes(&self) -> usize {
        self.reserved_bytes.load(Ordering::Relaxed)
    }

    fn acquire_pages<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        let pages = pages.next_power_of_two(); // FIXME
        let units = pages << (S::LOG_BYTES - Size4K::LOG_BYTES);
        let start = self.freelist.lock().allocate_cell(units)?.start;
        let start = Page::<S>::new(start);
        self.map_pages(start, pages);
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
