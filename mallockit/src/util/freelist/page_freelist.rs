use crate::{
    space::page_table::PageTable,
    util::{arena::Arena, *},
};
use std::{ops::Range, ptr::NonNull};

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
    page_table: PageTable,
    arena: Arena<Cell>,
}

unsafe impl<const NUM_SIZE_CLASS: usize> Sync for PageFreeList<NUM_SIZE_CLASS> {}
unsafe impl<const NUM_SIZE_CLASS: usize> Send for PageFreeList<NUM_SIZE_CLASS> {}

impl<const NUM_SIZE_CLASS: usize> InternalAbstractFreeList for PageFreeList<NUM_SIZE_CLASS> {
    const MIN_SIZE_CLASS: usize = 0;
    const NUM_SIZE_CLASS: usize = NUM_SIZE_CLASS;

    fn is_free(&self, unit: Unit, size_class: usize) -> bool {
        self.bst
            .get(self.unit_to_index(unit, size_class))
            .unwrap_or(false)
    }

    fn set_as_free(&mut self, unit: Unit, size_class: usize) {
        if cfg!(feature = "slow_assert") {
            debug_assert!(self.is_not_free_slow(unit));
        }
        let index = self.unit_to_index(unit, size_class);
        self.bst.set(index, true);
    }

    fn set_as_used(&mut self, unit: Unit, size_class: usize) {
        debug_assert!(self.is_free(unit, size_class));
        let index = self.unit_to_index(unit, size_class);
        self.bst.set(index, false);
        if cfg!(feature = "slow_assert") {
            debug_assert!(self.is_not_free_slow(unit));
        }
    }

    fn push_cell(&mut self, unit: Unit, size_class: usize) {
        let head = self.table[size_class];
        let cell = self.arena.alloc(Cell {
            prev: None,
            next: None,
            unit,
        });
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

    fn pop_cell(&mut self, size_class: usize) -> Option<Unit> {
        let head = self.table[size_class];
        if head.is_none() {
            None
        } else {
            let mut head_ptr = head.unwrap();
            let head = unsafe { head_ptr.as_mut() };
            let next = head.next;
            if let Some(mut next) = next {
                unsafe {
                    debug_assert_eq!(next.as_ref().prev, Some(head_ptr));
                    next.as_mut().prev = None;
                }
            }
            self.table[size_class] = next;
            let unit = head.unit;
            self.delete_pages(unit);
            self.arena.dealloc(head);
            Some(unit)
        }
    }

    fn remove_cell(&mut self, unit: Unit, size_class: usize) {
        let mut cell_ptr = self.unit_to_cell(unit);
        let cell = unsafe { cell_ptr.as_mut() };
        let next = cell.next;
        let prev = cell.prev;
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
        self.arena.dealloc(cell);
    }
}

impl<const NUM_SIZE_CLASS: usize> PageFreeList<NUM_SIZE_CLASS> {
    fn unit_to_address(&self, unit: Unit) -> Address {
        self.base + (*unit << Size4K::LOG_BYTES)
    }

    fn address_to_unit(&self, a: Address) -> Unit {
        Unit((a - self.base) >> Size4K::LOG_BYTES)
    }

    fn unit_to_cell(&self, unit: Unit) -> CellPtr {
        let ptr = self.page_table.get_pointer_meta(self.unit_to_address(unit));
        unsafe { NonNull::new_unchecked(ptr.as_mut_ptr()) }
    }

    fn delete_pages(&mut self, unit: Unit) {
        self.page_table
            .delete_pages::<Size4K>(Page::new(self.unit_to_address(unit)), 1);
    }

    fn insert_pages(&mut self, unit: Unit, pointer_meta: Address) {
        let addr = self.unit_to_address(unit);
        self.page_table.insert_pages::<Size4K>(Page::new(addr), 1);
        self.page_table.set_pointer_meta(addr, pointer_meta);
    }
}

impl<const NUM_SIZE_CLASS: usize> PageFreeList<NUM_SIZE_CLASS> {
    pub fn new(base: Address) -> Self {
        Self {
            base,
            table: [None; NUM_SIZE_CLASS],
            bst: LazyBst::new(),
            page_table: PageTable::new(),
            arena: Arena::new(),
        }
    }

    pub fn allocate_cell(&mut self, units: usize) -> Option<Range<Address>> {
        let Range { start, end } = self.allocate_cell_unaligned_size(units)?;
        let start = self.unit_to_address(start);
        let end = self.unit_to_address(end);
        Some(start..end)
    }

    pub fn release_cell(&mut self, start: Address, units: usize) {
        let unit = self.address_to_unit(start);
        self.release_cell_unaligned_size(unit, units);
    }
}
