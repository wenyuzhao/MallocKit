use crate::util::{sys::raw_memory::RawMemory, *};
use std::{
    iter::Step,
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

pub static META_SPACE: MetaSpace = MetaSpace::new();

pub struct MetaSpace {
    committed_size: AtomicUsize,
}

impl MetaSpace {
    const fn new() -> Self {
        Self {
            committed_size: AtomicUsize::new(0),
        }
    }

    pub fn committed_size(&self) -> usize {
        self.committed_size.load(Ordering::SeqCst)
    }

    pub fn map<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        let size = pages << S::LOG_BYTES;
        let addr = RawMemory::map_anonymous(size).ok()?;
        self.committed_size
            .fetch_add(pages << S::LOG_BYTES, Ordering::SeqCst);
        let start = Page::new(addr);
        let end = Page::forward(start, pages);
        Some(start..end)
    }

    pub fn unmap<S: PageSize>(&self, start: Page<S>, pages: usize) {
        RawMemory::unmap(start.start(), pages << S::LOG_BYTES);
        self.committed_size
            .fetch_sub(pages << S::LOG_BYTES, Ordering::SeqCst);
    }
}
