use super::super::PAGE_REGISTRY;
use crate::util::*;
use std::ops::Range;

pub trait PageResource: Sized {
    fn reserved_bytes(&self) -> usize;

    fn acquire_pages<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>>;

    fn release_pages<S: PageSize>(&self, start: Page<S>);

    fn get_contiguous_pages<S: PageSize>(&self, start: Page<S>) -> usize {
        PAGE_REGISTRY.get_contiguous_pages(start.start())
    }
}
