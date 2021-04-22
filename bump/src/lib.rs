#![allow(incomplete_features)]
#![feature(impl_trait_in_bindings)]
#![feature(min_type_alias_impl_trait)]
#![feature(core_intrinsics)]
#![feature(const_fn)]
#![feature(const_raw_ptr_to_usize_cast)]
#![feature(thread_local)]

#[macro_use] extern crate malloctk;

use core::alloc::Layout;
use malloctk::{Mutator, Plan, util::{Address, AllocationArea, Lazy}};
use libc;



struct Bump;

impl Plan for Bump {
    type Mutator = BumpMutator;

    fn new() -> Self {
        Self
    }

    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        unsafe { *ptr.as_ptr::<Layout>().sub(1) }
    }
}

struct BumpMutator {
    allocation_area: AllocationArea,
    retry: bool,
}

impl BumpMutator {
    const fn new() -> Self {
        Self {
            allocation_area: AllocationArea::EMPTY,
            retry: false,
        }
    }

    #[cold]
    fn alloc_slow(&mut self, layout: Layout) -> Option<Address> {
        assert!(!self.retry);
        let page_size = 1usize << 12;
        let block_size = page_size * 8;
        let mmap_size = AllocationArea::align_up(usize::max(layout.size(), block_size) + std::mem::size_of::<Layout>(), page_size);
        let top = unsafe {
            let addr = libc::mmap(0 as _, mmap_size, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_SHARED | libc::MAP_ANONYMOUS, -1, 0);
            assert!(addr != libc::MAP_FAILED);
            Address::from(addr)
        };
        let limit = top + mmap_size;
        self.allocation_area = AllocationArea { top, limit };
        self.retry = true;
        let result = self.alloc(layout);
        self.retry = false;
        result
    }
}

impl Mutator for BumpMutator {
    type Plan = Bump;

    fn current() -> &'static mut Self {
        unsafe { &mut MUTATOR }
    }

    #[inline(always)]
    fn plan(&self) -> &'static Self::Plan {
        &PLAN
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        if let Some(ptr) = self.allocation_area.alloc(layout, true) {
            return Some(ptr)
        }
        self.alloc_slow(layout)
    }

    #[inline(always)]
    fn dealloc(&mut self, _ptr: Address, _layout: Layout) {}

}

static PLAN: Lazy<Bump> = Lazy::new(|| Bump::new());

#[thread_local]
static mut MUTATOR: BumpMutator = BumpMutator::new();

export_malloc_api!(PLAN);