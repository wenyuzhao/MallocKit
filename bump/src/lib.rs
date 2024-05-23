#![feature(thread_local)]
#![feature(allocator_api)]

extern crate mallockit;

use mallockit::{
    space::{immortal_space::*, *},
    util::*,
    Mutator, Plan,
};

const IMMORTAL_SPACE: SpaceId = SpaceId::DEFAULT;

#[mallockit::plan]
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

    fn get_layout(ptr: Address) -> Layout {
        debug_assert!(IMMORTAL_SPACE.contains(ptr));
        ImmortalSpace::get_layout(ptr)
    }
}

#[mallockit::mutator]
struct BumpMutator {
    bump: BumpAllocator,
}

impl Mutator for BumpMutator {
    type Plan = Bump;

    fn new() -> Self {
        Self {
            bump: BumpAllocator::new(&Self::plan().immortal),
        }
    }

    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        self.bump.alloc(layout)
    }

    fn dealloc(&mut self, _: Address) {}
}
