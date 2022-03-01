use super::super::SpaceId;
use super::PageResource;
use crate::util::memory::RawMemory;
use crate::util::*;
use atomic::Atomic;
use crossbeam::queue::SegQueue;
use std::iter::Step;
use std::{
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

pub struct BlockPageResource {
    pub id: SpaceId,
    log_bytes: usize,
    cursor: Atomic<Address>,
    highwater: Address,
    recycled_blocks: SegQueue<Address>,
    reserved_bytes: AtomicUsize,
}

impl BlockPageResource {
    pub fn new(id: SpaceId, log_bytes: usize) -> Self {
        debug_assert!(id.0 < 0b0000_1111);
        debug_assert!(log_bytes >= Size4K::LOG_BYTES);
        Self {
            id,
            log_bytes,
            cursor: Atomic::new(id.address_space().start),
            highwater: id.address_space().end,
            recycled_blocks: SegQueue::new(),
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
                true
            }
            _ => false,
        }
    }

    #[cold]
    fn acquire_block_slow<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        debug_assert!(self.log_bytes >= S::LOG_BYTES);
        debug_assert_eq!(pages, 1 << (self.log_bytes - S::LOG_BYTES));
        let block = self
            .cursor
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |a| {
                if a >= self.highwater {
                    None
                } else {
                    Some(a + (1usize << self.log_bytes))
                }
            });
        match block {
            Ok(addr) => {
                let start = Page::<S>::new(addr);
                if !self.map_pages(start, pages) {
                    return self.acquire_block_slow(pages); // Retry
                }
                let end = Step::forward(start, pages);
                return Some(start..end);
            }
            Err(_) => return None,
        }
    }
}

impl PageResource for BlockPageResource {
    #[inline(always)]
    fn reserved_bytes(&self) -> usize {
        self.reserved_bytes.load(Ordering::Relaxed)
    }

    fn acquire_pages<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        debug_assert!(self.log_bytes >= S::LOG_BYTES);
        debug_assert_eq!(pages, 1 << (self.log_bytes - S::LOG_BYTES));
        if let Some(addr) = self.recycled_blocks.pop() {
            let start = Page::<S>::new(addr);
            let end = Step::forward(start, pages);
            self.reserved_bytes
                .fetch_add(1 << self.log_bytes, Ordering::Relaxed);
            return Some(start..end);
        }
        if let Some(result) = self.acquire_block_slow(pages) {
            self.reserved_bytes
                .fetch_add(1 << self.log_bytes, Ordering::Relaxed);
            return Some(result);
        }
        None
    }

    fn release_pages<S: PageSize>(&self, start: Page<S>) {
        self.recycled_blocks.push(start.start());
        self.reserved_bytes
            .fetch_sub(1 << self.log_bytes, Ordering::Relaxed);
    }
}
