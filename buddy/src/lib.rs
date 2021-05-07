#![allow(incomplete_features)]
#![feature(impl_trait_in_bindings)]
#![feature(min_type_alias_impl_trait)]
#![feature(core_intrinsics)]
#![feature(const_fn)]
#![feature(const_raw_ptr_to_usize_cast)]
#![feature(thread_local)]
#![feature(allocator_api)]
#![feature(step_trait)]
#![feature(exclusive_range_pattern)]
#![feature(const_ptr_offset)]
#![feature(const_raw_ptr_deref)]
#![feature(const_mut_refs)]
#![feature(const_trait_impl)]
#![feature(const_fn_fn_ptr_basics)]

#[macro_use] extern crate mallockit;

use core::alloc::Layout;
use std::intrinsics::likely;
use mallockit::{Mutator, Plan, space::*, space::{freelist_space::*, large_object_space::*}, util::*};

const FREELIST_SPACE: SpaceId = SpaceId::DEFAULT;
const LARGE_OBJECT_SPACE: SpaceId = SpaceId::LARGE_OBJECT_SPACE;

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
    fn get_layout(&self, _: Address) -> Layout {
        unreachable!()
    }
}

// type FreeListKind = BitMapFreeList;
type FreeListKind = HeaderFreeList;

struct BuddyMutator {
    freelist: FreeListAllocator<FreeListKind>,
    los: LargeObjectAllocator,
}

impl BuddyMutator {
    const fn new() -> Self {
        Self {
            freelist: FreeListAllocator::<FreeListKind>::new(Lazy::new(|| &PLAN.freelist_space), FREELIST_SPACE),
            los: LargeObjectAllocator(Lazy::new(|| &PLAN.large_object_space)),
        }
    }
}

impl Mutator for BuddyMutator {
    type Plan = Buddy;

    #[inline(always)]
    fn current() -> &'static mut Self {
        unsafe { &mut MUTATOR }
    }

    #[inline(always)]
    fn plan(&self) -> &'static Self::Plan {
        &PLAN
    }

    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        if likely(FREELIST_SPACE.contains(ptr)) {
            self.freelist.get_layout(ptr)
        } else {
            self.los.get_layout(ptr)
        }
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        if likely(FreeListSpace::can_allocate::<FreeListKind>(layout)) {
            mallockit::stat::track_allocation(layout, false);
            self.freelist.alloc(layout)
        } else {
            mallockit::stat::track_allocation(layout, true);
            self.los.alloc(layout)
        }
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        if likely(FREELIST_SPACE.contains(ptr)) {
            mallockit::stat::track_deallocation(false);
            self.freelist.dealloc(ptr)
        } else {
            mallockit::stat::track_deallocation(true);
            self.los.dealloc(ptr)
        }
    }
}

static PLAN: Lazy<Buddy> = Lazy::new(|| Buddy::new());

#[thread_local]
static mut MUTATOR: BuddyMutator = BuddyMutator::new();

export_malloc_api!(PLAN);