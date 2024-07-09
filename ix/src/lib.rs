#![feature(thread_local)]
#![feature(step_trait)]
#![feature(allocator_api)]
#![feature(const_mut_refs)]
#![feature(const_trait_impl)]
#![feature(effects)]

extern crate mallockit;

mod block;
mod immix_space;
mod pool;

use immix_space::*;
use mallockit::{
    space::{large_object_space::*, *},
    util::*,
    Mutator, Plan,
};

const IMMIX_SPACE: SpaceId = SpaceId::DEFAULT;
const LARGE_OBJECT_SPACE: SpaceId = SpaceId::LARGE_OBJECT_SPACE;

#[mallockit::plan]
struct Immix {
    immix_space: ImmixSpace,
    large_object_space: LargeObjectSpace,
}

impl Plan for Immix {
    type Mutator = ImmixMutator;

    fn new() -> Self {
        Self {
            immix_space: ImmixSpace::new(IMMIX_SPACE),
            large_object_space: LargeObjectSpace::new(LARGE_OBJECT_SPACE),
        }
    }

    fn get_layout(ptr: Address) -> Layout {
        debug_assert!(IMMIX_SPACE.contains(ptr) || LARGE_OBJECT_SPACE.contains(ptr));
        if IMMIX_SPACE.contains(ptr) {
            ImmixSpace::get_layout(ptr)
        } else {
            Self::get().large_object_space.get_layout::<Size4K>(ptr)
        }
    }
}

#[mallockit::mutator]
struct ImmixMutator {
    hoard: ImmixAllocator,
    los: LargeObjectAllocator<Size4K, { 1 << 31 }, { 16 << 20 }>,
    _padding: [usize; 8],
}

impl Mutator for ImmixMutator {
    type Plan = Immix;

    fn new() -> Self {
        Self {
            hoard: ImmixAllocator::new(&Self::plan().immix_space, IMMIX_SPACE),
            los: LargeObjectAllocator::new(&Self::plan().large_object_space),
            _padding: [0; 8],
        }
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        if ImmixSpace::can_allocate(layout) {
            mallockit::stat::track_allocation(layout, false);
            self.hoard.alloc(layout)
        } else {
            mallockit::stat::track_allocation(layout, true);
            self.los.alloc(layout)
        }
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        debug_assert!(IMMIX_SPACE.contains(ptr) || LARGE_OBJECT_SPACE.contains(ptr));
        if IMMIX_SPACE.contains(ptr) {
            mallockit::stat::track_deallocation(false);
            self.hoard.dealloc(ptr)
        } else {
            mallockit::stat::track_deallocation(false);
            self.los.dealloc(ptr)
        }
    }
}
