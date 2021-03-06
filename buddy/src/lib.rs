#![allow(incomplete_features)]
#![feature(type_alias_impl_trait)]
#![feature(thread_local)]
#![feature(core_intrinsics)]

extern crate mallockit;

use core::alloc::Layout;
use mallockit::{
    space::*,
    space::{freelist_space::*, large_object_space::*},
    util::*,
    Mutator, Plan,
};
use std::intrinsics::likely;

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

    #[inline(always)]
    fn get_layout(ptr: Address) -> Layout {
        debug_assert!(FREELIST_SPACE.contains(ptr) || LARGE_OBJECT_SPACE.contains(ptr));
        if likely(FREELIST_SPACE.contains(ptr)) {
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

impl BuddyMutator {
    const fn new() -> Self {
        Self {
            freelist: FreeListAllocator::new(
                Lazy::new(|| &Self::plan().freelist_space),
                FREELIST_SPACE,
            ),
            los: LargeObjectAllocator::new(Lazy::new(|| &Self::plan().large_object_space)),
        }
    }
}

impl Mutator for BuddyMutator {
    type Plan = Buddy;
    const NEW: Self = Self::new();

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        if likely(FreeListSpace::can_allocate(layout)) {
            mallockit::stat::track_allocation(layout, false);
            self.freelist.alloc(layout)
        } else {
            mallockit::stat::track_allocation(layout, true);
            self.los.alloc(layout)
        }
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        debug_assert!(FREELIST_SPACE.contains(ptr) || LARGE_OBJECT_SPACE.contains(ptr));
        if likely(FREELIST_SPACE.contains(ptr)) {
            mallockit::stat::track_deallocation(false);
            self.freelist.dealloc(ptr)
        } else {
            mallockit::stat::track_deallocation(false);
            self.los.dealloc(ptr)
        }
    }
}
