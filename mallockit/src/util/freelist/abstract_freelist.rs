use crate::util::*;
use std::{
    intrinsics::unlikely,
    ops::{Deref, Range},
};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(C)]
pub(super) struct Unit(pub(super) usize);

impl const Deref for Unit {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl Unit {
    const fn parent(&self, size_class: usize) -> Self {
        Self(self.0 & !(1 << size_class))
    }

    const fn is_aligned(&self, size_class: usize) -> bool {
        (self.0 & ((1usize << size_class) - 1)) == 0
    }

    pub(crate) const fn sibling(&self, size_class: usize) -> Self {
        Self(self.0 ^ (1 << size_class))
    }
}

pub(super) struct LazyBst {
    bits: Vec<Option<Page>, System>,
}

impl LazyBst {
    pub(super) const fn new() -> Self {
        Self {
            bits: Vec::new_in(System),
        }
    }

    #[inline(always)]
    fn needs_resize(&self, index: BstIndex) -> bool {
        let index = *index;
        let byte_index = index >> 3;
        let page_index = byte_index >> Size2M::LOG_BYTES;
        page_index >= self.bits.len() || self.bits[page_index].is_none()
    }

    #[cold]
    fn resize(&mut self, index: BstIndex) {
        let index = *index;
        let byte_index = index >> 3;
        let page_index = byte_index >> Size2M::LOG_BYTES;
        if page_index >= self.bits.len() {
            self.bits.resize((page_index + 1).next_power_of_two(), None);
        }
        if self.bits[page_index].is_none() {
            let page = unsafe {
                let addr = libc::mmap(
                    0 as _,
                    Size2M::BYTES,
                    libc::PROT_READ | libc::PROT_WRITE,
                    libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                    -1,
                    0,
                );
                let addr = Address::from(addr);
                Page::new(addr)
            };
            debug_assert!(page.is_zeroed());
            self.bits[page_index] = Some(page);
        }
    }

    #[inline(always)]
    fn get_bit_location(&self, index: BstIndex) -> Option<(Address, usize)> {
        let index = *index;
        let byte_index = index >> 3;
        let page_index = byte_index >> Size2M::LOG_BYTES;
        if page_index >= self.bits.len() {
            return None;
        }
        let byte_offset_in_page = byte_index & Page::<Size2M>::MASK;
        let bit_offset_in_byte = index & 0b111;
        let addr = self.bits[page_index]?.start() + byte_offset_in_page;
        Some((addr, bit_offset_in_byte))
    }

    #[inline(always)]
    pub(super) fn get(&self, index: BstIndex) -> Option<bool> {
        let (addr, bit_index) = self.get_bit_location(index)?;
        Some(unsafe { (addr.load::<u8>() & (1 << bit_index)) != 0 })
    }

    #[inline(always)]
    pub(super) fn set(&mut self, index: BstIndex, value: bool) {
        if unlikely(self.needs_resize(index)) {
            self.resize(index);
        }
        let (addr, bit_index) = self.get_bit_location(index).unwrap();
        if value {
            unsafe {
                addr.store::<u8>(addr.load::<u8>() | (1 << bit_index));
            }
        } else {
            unsafe {
                addr.store::<u8>(addr.load::<u8>() & !(1 << bit_index));
            }
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(C)]
pub(super) struct BstIndex(usize);

impl const Deref for BstIndex {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Manage allocation of 0..(1 << NUM_SIZE_CLASS) units
pub(super) trait InternalAbstractFreeList: Sized {
    const MIN_SIZE_CLASS: usize;
    const NUM_SIZE_CLASS: usize;
    const NON_COALESCEABLE_SIZE_CLASS_THRESHOLD: usize = Self::NUM_SIZE_CLASS - 1;

    fn is_free(&self, unit: Unit, size_class: usize) -> bool;
    fn set_as_free(&mut self, unit: Unit, size_class: usize);
    fn set_as_used(&mut self, unit: Unit, size_class: usize);
    fn pop_cell(&mut self, size_class: usize) -> Option<Unit>;
    fn push_cell(&mut self, unit: Unit, size_class: usize);
    fn remove_cell(&mut self, unit: Unit, size_class: usize);

    #[inline(always)]
    fn unit_to_index(&self, unit: Unit, size_class: usize) -> BstIndex {
        let start = 1 << (Self::NUM_SIZE_CLASS - size_class - 1);
        let index = *unit >> size_class;
        debug_assert!(start + index < (1 << (Self::NUM_SIZE_CLASS - size_class)));
        BstIndex(start + index)
    }

    #[inline(always)]
    fn push(&mut self, unit: Unit, size_class: usize) {
        self.push_cell(unit, size_class);
        self.set_as_free(unit, size_class);
    }

    #[inline(always)]
    fn pop(&mut self, size_class: usize) -> Option<Unit> {
        let unit = self.pop_cell(size_class)?;
        self.set_as_used(unit, size_class);
        Some(unit)
    }

    #[inline(always)]
    fn remove(&mut self, unit: Unit, size_class: usize) {
        self.remove_cell(unit, size_class);
        self.set_as_used(unit, size_class);
    }

    fn split_cell(&mut self, parent: Unit, parent_size_class: usize) -> (Unit, Unit) {
        let child_size_class = parent_size_class - 1;
        let unit1 = parent;
        let unit2 = unit1.sibling(child_size_class);
        debug_assert!(!self.is_free(unit1, child_size_class)); // child#0 is used
        debug_assert!(!self.is_free(unit2, child_size_class)); // child#1 is used
        (unit1, unit2)
    }

    #[cold]
    fn allocate_aligned_units_slow(&mut self, request_size_class: usize) -> Option<Unit> {
        for size_class in request_size_class..=Self::NON_COALESCEABLE_SIZE_CLASS_THRESHOLD {
            if let Some(unit) = self.pop(size_class) {
                debug_assert!(!self.is_free(unit, size_class));
                let parent = unit;
                for parent_size_class in ((request_size_class + 1)..=size_class).rev() {
                    debug_assert!(!self.is_free(parent, parent_size_class)); // parent is used
                                                                             // Split into two
                    let (unit1, unit2) = self.split_cell(parent, parent_size_class);
                    let child_size_class = parent_size_class - 1;
                    // Add second cell to list
                    debug_assert!(child_size_class < Self::NUM_SIZE_CLASS);
                    self.push(unit2, child_size_class);
                    debug_assert!(!self.is_free(parent, parent_size_class)); // parent is used
                    debug_assert!(!self.is_free(unit1, child_size_class)); // child#0 is used
                    debug_assert!(self.is_free(unit2, child_size_class)); // child#1 is free
                }
                return Some(unit);
            }
        }
        None
    }

    #[inline(always)]
    fn allocate_aligned_units(&mut self, size_class: usize) -> Option<Unit> {
        if size_class > Self::NON_COALESCEABLE_SIZE_CLASS_THRESHOLD {
            return None;
        }
        if let Some(unit) = self.pop(size_class) {
            debug_assert!(!self.is_free(unit, size_class));
            return Some(unit);
        }
        self.allocate_aligned_units_slow(size_class)
    }

    fn is_not_free_slow(&self, unit: Unit) -> bool {
        assert!(cfg!(feature = "slow_assert"));
        for sz in 0..Self::NUM_SIZE_CLASS {
            if self.is_free(unit, sz) {
                return true;
            }
        }
        false
    }

    #[inline(always)]
    fn release_aligned_units(
        &mut self,
        mut unit: Unit,
        mut size_class: usize,
        force_no_coalesce: bool,
    ) {
        loop {
            debug_assert!(unit.is_aligned(size_class));
            debug_assert!(size_class < Self::NUM_SIZE_CLASS);
            let sibling = unit.sibling(size_class);
            debug_assert!(!self.is_free(unit, size_class));
            if unlikely(
                !force_no_coalesce
                    && size_class < Self::NON_COALESCEABLE_SIZE_CLASS_THRESHOLD
                    && self.is_free(sibling, size_class),
            ) {
                let parent = unit.parent(size_class);
                debug_assert!(
                    !self.is_free(parent, size_class + 1),
                    "{:?} {}",
                    parent,
                    size_class
                ); // parent is used
                   // Remove sibling from list
                self.remove(sibling, size_class);
                debug_assert!(!self.is_free(unit, size_class)); // unit is used
                debug_assert!(!self.is_free(sibling, size_class)); // sibling is used
                                                                   // Merge unit and sibling
                unit = parent;
                size_class += 1;
            } else {
                debug_assert!(size_class < Self::NUM_SIZE_CLASS);
                self.push(unit, size_class);
                debug_assert!(self.is_free(unit, size_class)); // unit is free
                return;
            }
        }
    }

    #[inline(always)]
    fn size_class(units: usize) -> usize {
        let a = units.next_power_of_two().trailing_zeros() as _;
        let b = Self::MIN_SIZE_CLASS;
        usize::max(a, b)
    }

    /// Allocate a cell with a power-of-two size, and aligned to the size.
    #[inline(always)]
    fn allocate_cell_aligned_size(&mut self, units: usize) -> Option<Range<Unit>> {
        debug_assert!(units.is_power_of_two());
        let size_class = <Self as InternalAbstractFreeList>::size_class(units);
        let start = self.allocate_aligned_units(size_class)?;
        Some(start..Unit(*start + units))
    }

    #[inline(always)]
    fn release_cell_aligned_size(&mut self, start: Unit, units: usize) {
        debug_assert!(units.is_power_of_two());
        debug_assert!(*start & (units - 1) == 0);
        let size_class = <Self as InternalAbstractFreeList>::size_class(units);
        self.release_aligned_units(start, size_class, false);
    }

    /// Allocate a cell with a power-of-two alignment.
    #[inline(always)]
    fn allocate_cell_unaligned_size(&mut self, units: usize) -> Option<Range<Unit>> {
        let units =
            (units + ((1 << Self::MIN_SIZE_CLASS) - 1)) & !((1 << Self::MIN_SIZE_CLASS) - 1);
        let size_class = Self::size_class(units);
        let start = self.allocate_aligned_units(size_class)?;
        if unlikely(units == (1 << size_class)) {
            let free_units = (1 << size_class) - units;
            let free_start = Unit(*start + units);
            self.release_cell_unaligned_size(free_start, free_units);
        }
        Some(start..Unit(*start + units))
    }

    #[inline(always)]
    fn release_cell_unaligned_size(&mut self, mut start: Unit, mut units: usize) {
        let limit = Unit(*start + units);
        while *start < *limit {
            let curr_size_class = Self::size_class(units);
            let prev_size_class = if units == (1 << curr_size_class) {
                curr_size_class
            } else {
                curr_size_class - 1
            };
            let size_class = usize::min(prev_size_class, (*start).trailing_zeros() as usize);
            let size = 1usize << size_class;
            let end = Unit(*start + size);
            debug_assert_eq!((*start & (size - 1)), 0);
            debug_assert!(*end <= *limit);
            self.release_aligned_units(start, size_class, false);
            start = end;
            units = *limit - *end;
        }
        debug_assert_eq!(start, limit);
    }
}
