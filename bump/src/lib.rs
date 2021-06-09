#![allow(incomplete_features)]
#![feature(impl_trait_in_bindings)]
#![feature(min_type_alias_impl_trait)]
#![feature(core_intrinsics)]
#![feature(const_raw_ptr_to_usize_cast)]
#![feature(thread_local)]
#![feature(allocator_api)]
#![feature(const_ptr_offset)]
#![feature(const_raw_ptr_deref)]
#![feature(const_mut_refs)]
#![feature(const_fn_fn_ptr_basics)]

#[macro_use]
extern crate mallockit;

use core::alloc::Layout;
use mallockit::{space::immortal_space::*, space::*, util::*, Mutator, Plan};

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
        AllocationArea::load_layout(ptr)
    }
}

struct BumpMutator {
    bump: BumpAllocator,
}

impl BumpMutator {
    const fn new() -> Self {
        Self {
            bump: BumpAllocator::new(Lazy::new(|| &PLAN.immortal)),
        }
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
        AllocationArea::load_layout(ptr)
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        self.bump.alloc(layout)
    }

    #[inline(always)]
    fn dealloc(&mut self, _: Address) {}
}

static PLAN: Lazy<Bump> = Lazy::new(|| Bump::new());

#[thread_local]
static mut MUTATOR: BumpMutator = BumpMutator::new();

export_malloc_api!(PLAN);
