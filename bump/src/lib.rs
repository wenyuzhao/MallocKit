#![allow(incomplete_features)]
#![feature(impl_trait_in_bindings)]
#![feature(min_type_alias_impl_trait)]
#![feature(core_intrinsics)]
#![feature(const_fn)]
#![feature(const_raw_ptr_to_usize_cast)]
#![feature(thread_local)]
#![feature(allocator_api)]

#[macro_use] extern crate malloctk;

use core::alloc::Layout;
use malloctk::{Mutator, Plan, space::*, space::immortal_space::ImmortalSpace, util::{Address, AllocationArea, Lazy}};

const IMMORTAL_SPACE: SpaceId = SpaceId::DEFAULT;

struct Bump {
    immortal: ImmortalSpace,
}

impl Plan for Bump {
    type Mutator = BumpMutator;

    fn new() -> Self {
        Self {
            immortal: ImmortalSpace::new(IMMORTAL_SPACE),
        }
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
        let alloc_size = AllocationArea::align_up(usize::max(layout.size(), block_size) + std::mem::size_of::<Layout>(), page_size);
        let alloc_pages = alloc_size >> 12;
        let top = PLAN.immortal.acquire(alloc_pages)?;
        let limit = top + alloc_size;
        self.allocation_area = AllocationArea { top, limit };
        self.retry = true;
        let result = self.alloc(layout);
        self.retry = false;
        result
    }
}

impl Mutator for BumpMutator {
    type Plan = Bump;

    #[inline(always)]
    fn current() -> &'static mut Self {
        unsafe { &mut MUTATOR }
    }

    #[inline(always)]
    fn plan(&self) -> &'static Self::Plan {
        &PLAN
    }

    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        unsafe { *ptr.as_ptr::<Layout>().sub(1) }
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