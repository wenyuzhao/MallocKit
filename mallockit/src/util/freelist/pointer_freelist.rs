use std::{intrinsics::unlikely, marker::PhantomData, ptr::NonNull};
use crate::util::*;

use super::abstract_freelist::*;



#[derive(Debug)]
#[repr(C)]
pub struct Cell {
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

pub trait AddressSpaceConfig: Sized {
    const LOG_MIN_ALIGNMENT: usize;
    const LOG_COVERAGE: usize;
    const LOG_MAX_CELL_SIZE: usize = Self::LOG_COVERAGE;
    const NUM_SIZE_CLASS: usize = Self::LOG_COVERAGE + 1 - Self::LOG_MIN_ALIGNMENT;
}

/// Manage allocation of 0..(1 << NUM_SIZE_CLASS) units
pub struct PointerFreeList<Config: AddressSpaceConfig> where [Option<CellPtr>; Config::NUM_SIZE_CLASS]: Sized {
    base: Address,
    table: [Option<CellPtr>; Config::NUM_SIZE_CLASS],
    bst: LazyBst,
    phantom: PhantomData<Config>
}

impl<Config: AddressSpaceConfig>  InternalAbstractFreeList for PointerFreeList<Config> where [Option<CellPtr>; Config::NUM_SIZE_CLASS]: Sized {
    const MIN_SIZE_CLASS: usize = 1;
    const NUM_SIZE_CLASS: usize = Config::NUM_SIZE_CLASS;
    const NON_COALESCEABLE_SIZE_CLASS_THRESHOLD: usize = Config::LOG_MAX_CELL_SIZE - Config::LOG_MIN_ALIGNMENT;

    #[inline(always)]
    fn bst(&self) -> &LazyBst {
        &self.bst
    }
    #[inline(always)]
    fn bst_mut(&mut self) -> &mut LazyBst {
        &mut self.bst
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
    }
}

impl<Config: AddressSpaceConfig> PointerFreeList<Config> where [Option<CellPtr>; Config::NUM_SIZE_CLASS]: Sized {
    pub const fn new(base: Address) -> Self {
        debug_assert!(std::mem::size_of::<Cell>() == 16);
        Self {
            base,
            table: [None; Config::NUM_SIZE_CLASS],
            bst: LazyBst::new(),
            phantom: PhantomData,
        }
    }

    #[inline(always)]
    fn unit_to_cell(&self, unit: Unit) -> CellPtr {
        unsafe { NonNull::new_unchecked((self.base + (*unit << Config::LOG_MIN_ALIGNMENT)).as_mut_ptr()) }
    }

    #[inline(always)]
    fn cell_to_unit(&self, cell: CellPtr) -> Unit {
        Unit((Address::from(cell.as_ptr()) - self.base) >> Config::LOG_MIN_ALIGNMENT)
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
            for i in 0..Config::NUM_SIZE_CLASS {
                if self.is_on_current_list_slow(unit, Some(i)) {
                    count += 1;
                }
            }
            debug_assert!(count <= 1, "{}", count);
            count != 0
        }
    }
}

impl<Config: AddressSpaceConfig> AbstractFreeList for PointerFreeList<Config> where [Option<CellPtr>; Config::NUM_SIZE_CLASS]: Sized {
    #[inline(always)]
    fn unit_to_value(&self, unit: Unit) -> Address {
        Address::from(self.unit_to_cell(unit).as_ptr())
    }

    #[inline(always)]
    fn value_to_unit(&self, value: Address) -> Unit {
        self.cell_to_unit(unsafe { NonNull::new_unchecked(value.as_mut_ptr()) })
    }

    #[inline(always)]
    fn process_input_units(&self, units: usize) -> usize {
        units >> Config::LOG_MIN_ALIGNMENT
    }
}
