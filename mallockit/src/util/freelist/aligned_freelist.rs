use std::{intrinsics::unlikely, marker::PhantomData, ptr::NonNull};
use crate::util::*;
use super::unaligned_freelist::AddressSpaceConfig;
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



/// Manage allocation of 0..(1 << NUM_SIZE_CLASS) units
pub struct AlignedFreeList<Config: AddressSpaceConfig> where [Option<CellPtr>; Config::NUM_SIZE_CLASS]: Sized {
    base: Address,
    table: [Option<CellPtr>; Config::NUM_SIZE_CLASS],
    bst: LazyBst,
    phantom: PhantomData<Config>
}

impl<Config: AddressSpaceConfig>  InternalAbstractFreeList for AlignedFreeList<Config> where [Option<CellPtr>; Config::NUM_SIZE_CLASS]: Sized {
    const MIN_SIZE_CLASS: usize = 1;
    const NUM_SIZE_CLASS: usize = Config::NUM_SIZE_CLASS;
    const NON_COALESCEABLE_SIZE_CLASS_THRESHOLD: usize = Config::LOG_MAX_CELL_SIZE - Config::LOG_MIN_ALIGNMENT;

    #[inline(always)]
    fn is_free(&self, unit: Unit, size_class: usize) -> bool {
        self.bst.get(self.unit_to_index(unit, size_class)).unwrap_or(false)
    }

    #[inline(always)]
    fn set_as_free(&mut self, unit: Unit, size_class: usize) {
        if cfg!(feature="slow_assert") {
            debug_assert!(self.is_not_free_slow(unit));
        }
        let index = self.unit_to_index(unit, size_class);
        self.bst.set(index, true);
    }

    #[inline(always)]
    fn set_as_used(&mut self, unit: Unit, size_class: usize) {
        debug_assert!(self.is_free(unit, size_class));
        let index = self.unit_to_index(unit, size_class);
        self.bst.set(index, false);
        if cfg!(feature="slow_assert") {
            debug_assert!(self.is_not_free_slow(unit));
        }
    }

    #[inline(always)]
    fn push_cell(&mut self, unit: Unit, size_class: usize) {
        if cfg!(feature="slow_assert") {
            debug_assert!(!self.is_on_current_list_slow(unit, None));
        }
        let head = self.table[size_class];
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
        let head_opt = self.table[size_class];
        if unlikely(head_opt.is_none()) {
            return None;
        } else {
            let mut head_ptr = head_opt.unwrap();
            let head = unsafe { head_ptr.as_mut() };
            let next = head.next;
            if let Some(mut next) = next {
                unsafe {
                    debug_assert_eq!(next.as_ref().prev, head_opt);
                    next.as_mut().prev = None;
                }
            }
            self.table[size_class] = next;
            debug_assert!(head.prev.is_none());
            return Some(self.cell_to_unit(head_ptr));
        }
    }

    #[inline(always)]
    fn remove_cell(&mut self, unit: Unit, size_class: usize) {
        let mut cell_ptr = self.unit_to_cell(unit);
        let cell = unsafe { cell_ptr.as_mut() };
        let next = cell.next;
        let prev = cell.prev;
        if let Some(mut prev) = prev {
            unsafe {
                debug_assert!(prev.as_ref().next == Some(cell_ptr));
                prev.as_mut().next = next;
            }
        } else {
            debug_assert!(self.table[size_class] == Some(cell_ptr));
            self.table[size_class] = next;
        }
        if let Some(mut next) = next {
            unsafe {
                debug_assert!(next.as_ref().prev == Some(cell_ptr));
                next.as_mut().prev = prev;
            }
        }
    }
}

impl<Config: AddressSpaceConfig> AlignedFreeList<Config> where [Option<CellPtr>; Config::NUM_SIZE_CLASS]: Sized {
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

    #[inline(always)]
    pub fn pop_raw_cell(&mut self, log_size: usize) -> Option<Address> {
        let size_class = <Self as InternalAbstractFreeList>::size_class(self.process_input_units(1 << log_size));
        let unit = self.pop(size_class)?;
        Some(self.unit_to_value(unit))
    }
}

impl<Config: AddressSpaceConfig> AlignedAbstractFreeList for AlignedFreeList<Config> where [Option<CellPtr>; Config::NUM_SIZE_CLASS]: Sized {
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
