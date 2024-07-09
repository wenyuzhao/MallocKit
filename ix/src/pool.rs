use crate::{block::Block, immix_space::ImmixSpace};
use mallockit::{
    space::page_resource::MemRegion,
    util::{mem::size_class::SizeClass, Address},
    Plan,
};
use spin::{relax::Yield, MutexGuard};

type Mutex<T> = spin::mutex::Mutex<T, Yield>;

pub struct Pool {
    pub global: bool,
    head: Option<Block>,
}

impl Drop for Pool {
    fn drop(&mut self) {}
}

impl Pool {
    const MAX_BINS: usize = 32;

    pub const fn new(global: bool) -> Self {
        Self { global, head: None }
    }

    pub const fn static_ref(&self) -> &'static Self {
        unsafe { &*(self as *const Self) }
    }
}
