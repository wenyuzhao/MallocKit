#![allow(incomplete_features)]
#![feature(impl_trait_in_bindings)]
#![feature(min_type_alias_impl_trait)]
#![feature(thread_local)]
#![feature(const_fn_fn_ptr_basics)]

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
    fn get_layout(&self, _ptr: Address) -> Layout {
        unreachable!()
    }
}

#[mallockit::mutator]
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
    const NEW: Self = Self::new();

    #[inline(always)]
    fn plan(&self) -> &'static Self::Plan {
        &PLAN
    }

    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        debug_assert!(IMMORTAL_SPACE.contains(ptr));
        AllocationArea::load_layout(ptr)
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        self.bump.alloc(layout)
    }

    #[inline(always)]
    fn dealloc(&mut self, _: Address) {}
}

#[mallockit::plan]
static PLAN: Lazy<Bump> = Lazy::new(|| Bump::new());
