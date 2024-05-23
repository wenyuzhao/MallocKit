#![feature(thread_local)]
#![feature(step_trait)]
#![feature(allocator_api)]

extern crate mallockit;

mod hoard_space;
mod pool;
mod super_block;

use hoard_space::*;
use mallockit::{
    space::{large_object_space::*, *},
    util::*,
    Mutator, Plan,
};

const HOARD_SPACE: SpaceId = SpaceId::DEFAULT;
const LARGE_OBJECT_SPACE: SpaceId = SpaceId::LARGE_OBJECT_SPACE;

#[mallockit::plan]
struct Hoard {
    hoard_space: HoardSpace,
    large_object_space: LargeObjectSpace,
}

impl Plan for Hoard {
    type Mutator = HoardMutator;

    fn new() -> Self {
        Self {
            hoard_space: HoardSpace::new(HOARD_SPACE),
            large_object_space: LargeObjectSpace::new(LARGE_OBJECT_SPACE),
        }
    }

    fn get_layout(ptr: Address) -> Layout {
        debug_assert!(HOARD_SPACE.contains(ptr) || LARGE_OBJECT_SPACE.contains(ptr));
        if HOARD_SPACE.contains(ptr) {
            HoardSpace::get_layout(ptr)
        } else {
            Self::get().large_object_space.get_layout::<Size4K>(ptr)
        }
    }
}

#[mallockit::mutator]
struct HoardMutator {
    hoard: HoardAllocator,
    los: LargeObjectAllocator<Size4K, { 1 << 31 }, { 16 << 20 }>,
}

impl Mutator for HoardMutator {
    type Plan = Hoard;

    fn new() -> Self {
        Self {
            hoard: HoardAllocator::new(&Self::plan().hoard_space, HOARD_SPACE),
            los: LargeObjectAllocator::new(&Self::plan().large_object_space),
        }
    }

    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        if HoardSpace::can_allocate(layout) {
            mallockit::stat::track_allocation(layout, false);
            self.hoard.alloc(layout)
        } else {
            mallockit::stat::track_allocation(layout, true);
            self.los.alloc(layout)
        }
    }

    fn dealloc(&mut self, ptr: Address) {
        debug_assert!(HOARD_SPACE.contains(ptr) || LARGE_OBJECT_SPACE.contains(ptr));
        if HOARD_SPACE.contains(ptr) {
            mallockit::stat::track_deallocation(false);
            self.hoard.dealloc(ptr)
        } else {
            mallockit::stat::track_deallocation(false);
            self.los.dealloc(ptr)
        }
    }
}
