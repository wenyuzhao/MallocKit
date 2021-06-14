#![allow(incomplete_features)]
#![feature(impl_trait_in_bindings)]
#![feature(min_type_alias_impl_trait)]
#![feature(thread_local)]
#![feature(const_fn_fn_ptr_basics)]
#![feature(core_intrinsics)]

extern crate mallockit;

use core::alloc::Layout;
use mallockit::{space::large_object_space::*, space::*, util::*, Mutator, Plan};

const LARGE_OBJECT_SPACE: SpaceId = SpaceId::LARGE_OBJECT_SPACE;

struct Sanity {
    large_object_space: LargeObjectSpace,
}

impl Plan for Sanity {
    type Mutator = SanityMutator;

    fn new() -> Self {
        Self {
            large_object_space: LargeObjectSpace::new(LARGE_OBJECT_SPACE),
        }
    }

    #[inline(always)]
    fn get_layout(&self, _: Address) -> Layout {
        unreachable!()
    }
}

#[mallockit::mutator]
struct SanityMutator {
    los: LargeObjectAllocator,
}

impl SanityMutator {
    const fn new() -> Self {
        Self {
            los: LargeObjectAllocator(Lazy::new(|| &PLAN.large_object_space)),
        }
    }
}

impl Mutator for SanityMutator {
    type Plan = Sanity;
    const NEW: Self = Self::new();

    #[inline(always)]
    fn plan(&self) -> &'static Self::Plan {
        &PLAN
    }

    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        debug_assert!(LARGE_OBJECT_SPACE.contains(ptr));
        self.los.get_layout(ptr)
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        self.los.alloc(layout)
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        debug_assert!(LARGE_OBJECT_SPACE.contains(ptr));
        self.los.dealloc(ptr)
    }
}

#[mallockit::plan]
static PLAN: Lazy<Sanity> = Lazy::new(|| Sanity::new());
