#![feature(thread_local)]
#![feature(allocator_api)]

extern crate mallockit;

use mallockit::{
    space::{freelist_space::*, large_object_space::*, *},
    util::*,
    Mutator, Plan,
};

const FREELIST_SPACE: SpaceId = SpaceId::DEFAULT;
const LARGE_OBJECT_SPACE: SpaceId = SpaceId::LARGE_OBJECT_SPACE;

#[mallockit::plan]
struct Buddy {
    freelist_space: FreeListSpace,
    large_object_space: LargeObjectSpace,
}

impl Plan for Buddy {
    type Mutator = BuddyMutator;

    fn new() -> Self {
        Self {
            freelist_space: FreeListSpace::new(FREELIST_SPACE),
            large_object_space: LargeObjectSpace::new(LARGE_OBJECT_SPACE),
        }
    }

    fn get_layout(ptr: Address) -> Layout {
        debug_assert!(FREELIST_SPACE.contains(ptr) || LARGE_OBJECT_SPACE.contains(ptr));
        if FREELIST_SPACE.contains(ptr) {
            FreeListSpace::get_layout(ptr)
        } else {
            Self::get().large_object_space.get_layout::<Size4K>(ptr)
        }
    }
}

#[mallockit::mutator]
struct BuddyMutator {
    freelist: FreeListAllocator,
    los: LargeObjectAllocator<Size4K>,
}

impl Mutator for BuddyMutator {
    type Plan = Buddy;

    fn new() -> Self {
        Self {
            freelist: FreeListAllocator::new::<FREELIST_SPACE>(Lazy::new(|| {
                &Self::plan().freelist_space
            })),
            los: LargeObjectAllocator::new(Lazy::new(|| &Self::plan().large_object_space)),
        }
    }

    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        if FreeListSpace::can_allocate(layout) {
            mallockit::stat::track_allocation(layout, false);
            self.freelist.alloc(layout)
        } else {
            mallockit::stat::track_allocation(layout, true);
            self.los.alloc(layout)
        }
    }

    fn dealloc(&mut self, ptr: Address) {
        debug_assert!(FREELIST_SPACE.contains(ptr) || LARGE_OBJECT_SPACE.contains(ptr));
        if FREELIST_SPACE.contains(ptr) {
            mallockit::stat::track_deallocation(false);
            self.freelist.dealloc(ptr)
        } else {
            mallockit::stat::track_deallocation(false);
            self.los.dealloc(ptr)
        }
    }
}
