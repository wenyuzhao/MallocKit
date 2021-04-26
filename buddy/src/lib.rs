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

#[macro_use] extern crate malloctk;

use core::alloc::Layout;
use std::intrinsics::likely;
use malloctk::{Mutator, Plan, space::*, space::{freelist_space::FreeListSpace, large_object_space::LargeObjectSpace}, util::*};

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

struct BuddyMutator {}

impl BuddyMutator {
    const fn new() -> Self {
        Self {}
    }
}

#[repr(C)]
struct Cell(Address, usize);

impl Cell {
    const fn from(ptr: Address) -> &'static mut Self {
        unsafe { &mut *ptr.as_mut_ptr::<Self>().sub(1) }
    }
    const fn set(&mut self, start: Address, size: usize) {
        self.0 = start;
        self.1 = size;
    }
    const fn size(&self) -> usize {
        self.1
    }
    const fn start(&self) -> Address {
        self.0
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
            let bytes = Cell::from(ptr).size();
            debug_assert_ne!(bytes, 0);
            debug_assert!(bytes.is_power_of_two(), "{:?}", bytes);
            unsafe { Layout::from_size_align_unchecked(bytes, bytes) }
        } else {
            let pages = PLAN.large_object_space.page_resource().get_contiguous_pages(Page::<Size2M>::new(ptr));
            let bytes = pages << Size2M::LOG_BYTES;
            debug_assert!(bytes.is_power_of_two());
            unsafe { Layout::from_size_align_unchecked(bytes, bytes) }
        }
    }

    #[inline(always)]
    fn alloc(&mut self, layout: Layout) -> Option<Address> {
        debug_assert!(layout.align().is_power_of_two());
        let (extended_layout, offset) = Layout::new::<Cell>().extend(layout).unwrap();
        if likely(extended_layout.size() < FreeListSpace::MAX_ALLOCATION_SIZE) {
            let size_class = FreeListSpace::size_class(extended_layout.size());
            let start = PLAN.freelist_space.alloc(size_class)?;
            let data_start = start + offset;
            Cell::from(data_start).set(start, 1 << size_class);
            debug_assert_eq!(usize::from(data_start) & (layout.align() - 1), 0);
            Some(data_start)
        } else {
            let size = layout.size();
            let pages = (size + Page::<Size2M>::MASK) >> Page::<Size2M>::LOG_BYTES;
            let start_page = PLAN.large_object_space.acquire::<Size2M>(pages)?.start;
            debug_assert_eq!(usize::from(start_page.start()) & (layout.align() - 1), 0);
            Some(start_page.start())
        }
    }

    #[inline(always)]
    fn dealloc(&mut self, ptr: Address) {
        if likely(FREELIST_SPACE.contains(ptr)) {
            let cell = Cell::from(ptr);
            let bytes = cell.size();
            debug_assert!(bytes.is_power_of_two());
            let size_class = FreeListSpace::size_class(bytes);
            PLAN.freelist_space.dealloc(cell.start(), size_class)
        } else {
            PLAN.large_object_space.release(Page::<Size2M>::new(ptr))
        }
    }
}

static PLAN: Lazy<Buddy> = Lazy::new(|| Buddy::new());

#[thread_local]
static mut MUTATOR: BuddyMutator = BuddyMutator::new();

export_malloc_api!(PLAN);