#![allow(incomplete_features)]
#![feature(type_alias_impl_trait)]
#![feature(thread_local)]

extern crate mallockit;

use core::alloc::Layout;
use mallockit::{space::immortal_space::*, space::*, util::*, Mutator, Plan};

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

impl BumpMutator {
    const fn new() -> Self {
        Self {
            bump: BumpAllocator::new(Lazy::new(|| &Self::plan().immortal)),
        }
    }
}

impl Mutator for BumpMutator {
    type Plan = Bump;
    const NEW: Self = Self::new();

    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        self.bump.alloc(layout)
    }

    fn dealloc(&mut self, _: Address) {}
}
