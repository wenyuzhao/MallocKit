use crate::util::*;
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

    pub fn map<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        let size = pages << S::LOG_BYTES;
        let addr = unsafe {
            libc::mmap(
                0 as _,
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if addr == libc::MAP_FAILED {
            None
        } else {
            #[cfg(target_os = "linux")]
            if cfg!(feature = "transparent_huge_page") && S::LOG_BYTES != Size4K::LOG_BYTES {
                unsafe {
                    libc::madvise(addr, size, libc::MADV_HUGEPAGE);
                }
            }
            self.committed_size
                .fetch_add(pages << S::LOG_BYTES, Ordering::SeqCst);
            let start = Page::new(Address::from(addr));
            let end = Page::forward(start, pages);
            Some(start..end)
        }
    }

    pub fn unmap<S: PageSize>(&self, start: Page<S>, pages: usize) {
        unsafe {
            libc::munmap(start.start().as_mut_ptr(), pages << S::LOG_BYTES);
        }
        self.committed_size
            .fetch_sub(pages << S::LOG_BYTES, Ordering::SeqCst);
    }
}
