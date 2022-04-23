#![allow(incomplete_features)]
#![feature(type_alias_impl_trait)]
#![feature(thread_local)]
#![feature(core_intrinsics)]
#![feature(const_mut_refs)]
#![feature(const_ptr_offset)]
#![feature(generic_const_exprs)]

#[allow(unused)]
#[macro_use]
extern crate mallockit;

mod block;
mod hoard_space;
mod pool;

use core::alloc::Layout;
use hoard_space::*;
use mallockit::{space::large_object_space::*, space::*, util::*, Mutator, Plan};
use std::intrinsics::likely;

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

    #[inline(always)]
    fn get_layout(ptr: Address) -> Layout {
        debug_assert!(HOARD_SPACE.contains(ptr) || LARGE_OBJECT_SPACE.contains(ptr));
        if likely(HOARD_SPACE.contains(ptr)) {
            HoardSpace::get_layout(ptr)
        } else {
            Self::get().large_object_space.get_layout::<Size4K>(ptr)
        }
    }
}

#[mallockit::mutator]
struct HoardMutator {
    hoard: HoardAllocator,
    los: LargeObjectAllocator<Size4K>,
}

impl HoardMutator {
    const fn new() -> Self {
        Self {
            hoard: HoardAllocator::new(Lazy::new(|| &Self::plan().hoard_space), HOARD_SPACE),
            los: LargeObjectAllocator::new(Lazy::new(|| &Self::plan().large_object_space)),
        }
    }
}

impl Mutator for HoardMutator {
    type Plan = Hoard;
    const NEW: Self = Self::new();

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        let x = if likely(HoardSpace::can_allocate(layout)) {
            mallockit::stat::track_allocation(layout, false);
            self.hoard.alloc(layout)
        } else {
            mallockit::stat::track_allocation(layout, true);
            self.los.alloc(layout)
        };
        debug_assert!(x.is_some());
        x
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        debug_assert!(HOARD_SPACE.contains(ptr) || LARGE_OBJECT_SPACE.contains(ptr));
        if likely(HOARD_SPACE.contains(ptr)) {
            mallockit::stat::track_deallocation(false);
            self.hoard.dealloc(ptr)
        } else {
            mallockit::stat::track_deallocation(false);
            self.los.dealloc(ptr)
        }
    }
}
