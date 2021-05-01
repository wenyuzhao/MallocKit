use std::{ops::Range, ptr::NonNull};
use crate::{space::page_table::PageTable, util::*};

use super::abstract_freelist::*;



#[derive(Debug)]
struct Cell {
    prev: Option<CellPtr>,
    next: Option<CellPtr>,
    unit: Unit,
}

type CellPtr = NonNull<Cell>;

/// Mange allocation of 0..(1 << NUM_SIZE_CLASS) units
pub struct PageFreeList<const NUM_SIZE_CLASS: usize> {
    base: Address,
    table: [Option<CellPtr>; NUM_SIZE_CLASS],
    bst: LazyBst,
    pub free_units: usize,
    pub total_units: usize,
    page_table: PageTable,
}


impl<const NUM_SIZE_CLASS: usize> InternalAbstractFreeList for PageFreeList<{NUM_SIZE_CLASS}> {
    const MIN_SIZE_CLASS: usize = 0;
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
        let head = self.table[size_class].take();
        let mut cell = Box::leak(Box::new_in(Cell { prev: None, next: None, unit }, System));
        let cell_ptr = unsafe { NonNull::new_unchecked(cell) };
        if let Some(mut head) = head {
            unsafe {
                debug_assert!(head.as_ref().prev.is_none());
                head.as_mut().prev = Some(cell_ptr);
            }
        }
        cell.next = head;
        self.table[size_class] = Some(cell_ptr);
        self.insert_pages(unit, Address::from(cell))
    }

    #[inline(always)]
    fn pop_cell(&mut self, size_class: usize) -> Option<Unit> {
        let head = self.table[size_class].take();
        if head.is_none() {
            return None;
        } else {
            let mut head_ptr = head.unwrap();
            let mut head = unsafe { Box::<Cell, System>::from_raw_in(head_ptr.as_mut(), System) };
            let next = head.next.take();
            if let Some(mut next) = next {
                unsafe {
                    debug_assert_eq!(next.as_ref().prev, Some(head_ptr));
                    next.as_mut().prev = None;
                }
            }
            self.table[size_class] = next;
            let unit = head.unit;
            self.delete_pages(unit);
            return Some(unit);
        }
    }

    #[inline(always)]
    fn remove_cell(&mut self, unit: Unit, size_class: usize) {
        let mut cell_ptr = self.unit_to_cell(unit);
        let mut cell = unsafe { Box::<Cell, System>::from_raw_in(cell_ptr.as_mut(), System) };
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
        self.delete_pages(unit);
    }
}

impl<const NUM_SIZE_CLASS: usize> PageFreeList<{NUM_SIZE_CLASS}> {
    pub fn new(base: Address) -> Self {
        Self {
            base,
            table: [None; NUM_SIZE_CLASS],
            bst: LazyBst::new(),
            free_units: 0,
            total_units: 0,
            page_table: PageTable::new(),
        }
    }

    #[inline(always)]
    fn unit_to_cell(&self, unit: Unit) -> CellPtr {
        unsafe { NonNull::new_unchecked((self.base + *unit).as_mut_ptr()) }
    }

    #[inline(always)]
    fn delete_pages(&mut self, unit: Unit) {
        self.page_table.delete_pages::<Size4K>(Page::new(self.base + (*unit << 12)), 1);
    }

    #[inline(always)]
    fn insert_pages(&mut self, unit: Unit, pointer_meta: Address) {
        self.page_table.insert_pages::<Size4K>(Page::new(self.base + (*unit << 12)), 1);
        self.page_table.set_pointer_meta(self.base + (*unit << 12), pointer_meta);
    }
}


impl<const NUM_SIZE_CLASS: usize> AbstractFreeList for PageFreeList<{NUM_SIZE_CLASS}> {
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