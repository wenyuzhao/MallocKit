mod block_page_resource;
mod freelist_page_resource;

pub use block_page_resource::*;
pub use freelist_page_resource::*;

use crate::util::*;
use std::ops::Range;

pub trait PageResource: Sized {
    fn reserved_bytes(&self) -> usize;

    fn acquire_pages<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>>;

    fn release_pages<S: PageSize>(&self, start: Page<S>);

    fn get_contiguous_pages<S: PageSize>(&self, _start: Page<S>) -> usize {
        unimplemented!()
    }
}
