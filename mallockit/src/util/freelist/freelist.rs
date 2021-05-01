use std::{intrinsics::unlikely, ops::Range, ptr::NonNull};
use crate::util::*;

use super::abstract_freelist::{AbstractFreeList, InternalAbstractFreeList, LazyBst, Unit};



#[derive(Debug)]
#[repr(C)]
struct Cell {
    prev: Option<CellPtr>,
    next: Option<CellPtr>,
}

impl PartialEq for Cell {
    #[inline(always)]
    fn eq(&self, other: &Cell) -> bool {
        self as *const _ == other as *const _
    }
}

type CellPtr = NonNull<Cell>;

/// Manage allocation of 0..(1 << NUM_SIZE_CLASS) units
pub struct FreeList<const NUM_SIZE_CLASS: usize> {
    base: Address,
    table: [Option<CellPtr>; NUM_SIZE_CLASS],
    bst: LazyBst,
    pub free_units: usize,
    pub total_units: usize,
}

impl<const NUM_SIZE_CLASS: usize> InternalAbstractFreeList for FreeList<{NUM_SIZE_CLASS}> {
    const MIN_SIZE_CLASS: usize = 4;
    const NUM_SIZE_CLASS: usize = NUM_SIZE_CLASS;

    #[inline(always)]
    fn bst(&self) -> &LazyBst {
        &self.bst
    }
    #[inline(always)]
    fn bst_mut(&mut self) -> &mut LazyBst {
        &mut self.bst
    }

    #[inline(always)]
    fn delta_free_units(&mut self, delta: isize) {
        if delta > 0 {
            self.free_units += delta as usize;
        } else {
            self.free_units -= delta as usize;
        }
    }

    #[inline(always)]
    fn push_cell(&mut self, unit: Unit, size_class: usize) {
        if cfg!(feature="slow_assert") {
            debug_assert!(!self.is_on_current_list_slow(unit, None));
        }
        let head = self.table[size_class].take();
        let mut cell_ptr = self.unit_to_cell(unit);
        let cell = unsafe { cell_ptr.as_mut() };
        cell.prev = None;
        if let Some(mut head) = head {
            unsafe {
                debug_assert!(head.as_ref().prev.is_none());
                head.as_mut().prev = Some(cell_ptr);
            }
        }
        cell.next = head;
        self.table[size_class] = Some(cell_ptr);
        debug_assert!(self.cell_to_unit(cell_ptr) == unit);
        self.set_as_free(unit, size_class);
        if cfg!(feature="slow_assert") {
            debug_assert!(self.is_on_current_list_slow(unit, Some(size_class)));
        }
    }

    #[inline(always)]
    fn pop_cell(&mut self, size_class: usize) -> Option<Unit> {
        let head_opt = self.table[size_class].take();
        if unlikely(head_opt.is_none()) {
            return None;
        } else {
            let mut head_ptr = head_opt.unwrap();
            let head = unsafe { head_ptr.as_mut() };
            let next = head.next.take();
            if let Some(mut next) = next {
                unsafe {
                    debug_assert_eq!(next.as_ref().prev, head_opt);
                    next.as_mut().prev = None;
                }
            }
            self.table[size_class] = next;
            debug_assert!(head.prev.is_none());
            debug_assert!(head.next.is_none());
            return Some(self.cell_to_unit(head_ptr));
        }
    }

    #[inline(always)]
    fn remove_cell(&mut self, unit: Unit, size_class: usize) {
        let mut cell_ptr = self.unit_to_cell(unit);
        let cell = unsafe { cell_ptr.as_mut() };
        let next = cell.next.take();
        let prev = cell.prev.take();
        if let Some(mut prev) = prev {
            unsafe {
                debug_assert_eq!(prev.as_ref().next, Some(cell_ptr));
                prev.as_mut().next = next;
            }
        } else {
            debug_assert_eq!(self.table[size_class], Some(cell_ptr));
            self.table[size_class] = next;
        }
        if let Some(mut next) = next {
            unsafe {
                debug_assert_eq!(next.as_ref().prev, Some(cell_ptr));
                next.as_mut().prev = prev;
            }
        }
        self.set_as_used(self.cell_to_unit(cell_ptr), size_class);
    }
}

impl<const NUM_SIZE_CLASS: usize> FreeList<{NUM_SIZE_CLASS}> {
    pub const fn new(base: Address) -> Self {
        debug_assert!(std::mem::size_of::<Cell>() == 16);
        Self {
            base,
            table: [None; NUM_SIZE_CLASS],
            bst: LazyBst::new(),
            free_units: 0,
            total_units: 0,
        }
    }

    #[inline(always)]
    fn unit_to_cell(&self, unit: Unit) -> CellPtr {
        unsafe { NonNull::new_unchecked((self.base + *unit).as_mut_ptr()) }
    }

    #[inline(always)]
    fn cell_to_unit(&self, cell: CellPtr) -> Unit {
        Unit(Address::from(cell.as_ptr()) - self.base)
    }

    fn is_on_current_list_slow(&self, unit: Unit, size_class: Option<usize>) -> bool {
        assert!(cfg!(feature="slow_assert"));
        if let Some(sz) = size_class {
            let mut head = self.table[sz];
            while let Some(c) = head {
                unsafe {
                    if self.cell_to_unit(c) == unit {
                        return true;
                    }
                    head = c.as_ref().next;
                }
            }
            false
        } else {
            let mut count = 0;
            for i in 0..NUM_SIZE_CLASS {
                if self.is_on_current_list_slow(unit, Some(i)) {
                    count += 1;
                }
            }
            debug_assert!(count <= 1, "{}", count);
            count != 0
        }
    }
}

impl<const NUM_SIZE_CLASS: usize> AbstractFreeList for FreeList<{NUM_SIZE_CLASS}> {
    #[inline(always)]
    fn size_class(units: usize) -> usize {
        <Self as InternalAbstractFreeList>::size_class(units)
    }

    /// Allocate a cell with a power-of-two size, and aligned to the size.
    #[inline(always)]
    fn allocate_cell_aligned(&mut self, units: usize) -> Option<Range<usize>> {
        <Self as InternalAbstractFreeList>::allocate_cell_aligned(self, units)
    }

    #[inline(always)]
    fn release_cell_aligned(&mut self, start: usize, units: usize) {
        <Self as InternalAbstractFreeList>::release_cell_aligned(self, start, units)
    }
}