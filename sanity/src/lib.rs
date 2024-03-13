#![feature(thread_local)]

extern crate mallockit;

use mallockit::{
    space::{large_object_space::*, *},
    util::*,
    Mutator, Plan,
};

const LARGE_OBJECT_SPACE: SpaceId = SpaceId::LARGE_OBJECT_SPACE;

#[mallockit::plan]
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

    fn get_layout(ptr: Address) -> Layout {
        debug_assert!(LARGE_OBJECT_SPACE.contains(ptr));
        Self::get().large_object_space.get_layout::<Size4K>(ptr)
    }
}

#[mallockit::mutator]
struct SanityMutator {
    los: LargeObjectAllocator,
}

impl Mutator for SanityMutator {
    type Plan = Sanity;

    fn new() -> Self {
        Self {
            los: LargeObjectAllocator::new(Lazy::new(|| &Self::plan().large_object_space)),
        }
    }

    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        self.los.alloc(layout)
    }

    fn dealloc(&mut self, ptr: Address) {
        debug_assert!(LARGE_OBJECT_SPACE.contains(ptr));
        self.los.dealloc(ptr)
    }
}
