use super::abstract_freelist::*;
use crate::util::*;
use std::{intrinsics::unlikely, marker::PhantomData, ops::Range, ptr::NonNull};

#[derive(Debug)]
#[repr(C)]
struct Cell {
    is_free: (u32, u32),
    owner: *mut u8,
    prev: Option<CellPtr>,
    next: Option<CellPtr>,
}

impl Cell {
    const HEADER_UNITS: usize = 1;
    const HEADER_BYTES: usize = Self::HEADER_UNITS << 3;
}

impl PartialEq for Cell {
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
pub struct IntrusiveFreeList<Config: AddressSpaceConfig>
where
    [(); Config::NUM_SIZE_CLASS]: Sized,
{
    #[allow(unused)]
    shared: bool,
    base: Address,
    table: [Option<CellPtr>; Config::NUM_SIZE_CLASS],
    phantom: PhantomData<Config>,
}

impl<Config: AddressSpaceConfig> InternalAbstractFreeList for IntrusiveFreeList<Config>
where
    [(); Config::NUM_SIZE_CLASS]: Sized,
{
    const MIN_SIZE_CLASS: usize = 2;
    const NUM_SIZE_CLASS: usize = Config::NUM_SIZE_CLASS;
    const NON_COALESCEABLE_SIZE_CLASS_THRESHOLD: usize =
        Config::LOG_MAX_CELL_SIZE - Config::LOG_MIN_ALIGNMENT;

    fn is_free(&self, unit: Unit, size_class: usize) -> bool {
        let cell = unsafe { self.unit_to_cell(unit).as_ref() };
        cell.is_free == (1, size_class as u32) && cell.owner == self as *const _ as _
    }

    fn set_as_free(&mut self, unit: Unit, size_class: usize) {
        unsafe { self.unit_to_cell(unit).as_mut().is_free = (1, size_class as u32) }
    }

    fn set_as_used(&mut self, unit: Unit, _size_class: usize) {
        unsafe { self.unit_to_cell(unit).as_mut().is_free = (0, 0) }
    }

    fn split_cell(&mut self, parent: Unit, parent_size_class: usize) -> (Unit, Unit) {
        let child_size_class = parent_size_class - 1;
        let unit1 = parent;
        let unit2 = unit1.sibling(child_size_class);
        self.set_as_used(unit1, child_size_class);
        self.set_as_used(unit2, child_size_class);
        (unit1, unit2)
    }

    fn push_cell(&mut self, unit: Unit, size_class: usize) {
        let head = self.table[size_class];
        let mut cell_ptr = self.unit_to_cell(unit);
        let cell = unsafe { cell_ptr.as_mut() };
        cell.prev = None;
        cell.owner = self as *const _ as _;
        cell.is_free = (1, size_class as _);
        if let Some(mut head) = head {
            unsafe {
                debug_assert!(head.as_ref().prev.is_none());
                head.as_mut().prev = Some(cell_ptr);
            }
        }
        cell.next = head;
        self.table[size_class] = Some(cell_ptr);
        debug_assert!(self.cell_to_unit(cell_ptr) == unit);
    }

    fn pop_cell(&mut self, size_class: usize) -> Option<Unit> {
        let head_opt = self.table[size_class];
        if unlikely(head_opt.is_none()) {
            return None;
        } else {
            debug_assert!(head_opt.is_some());
            let mut head_ptr = unsafe { head_opt.unwrap_unchecked() };
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
            head.is_free = (0, 0);
            head.owner = 0 as _;
            let unit = self.cell_to_unit(head_ptr);
            return Some(unit);
        }
    }

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
        } else if self.table[size_class] == Some(cell_ptr) {
            self.table[size_class] = next;
        }
        if let Some(mut next) = next {
            unsafe {
                debug_assert!(next.as_ref().prev == Some(cell_ptr));
                next.as_mut().prev = prev;
            }
        }
        cell.is_free = (0, 0);
        cell.owner = 0 as _;
    }
}

impl<Config: AddressSpaceConfig> IntrusiveFreeList<Config>
where
    [(); Config::NUM_SIZE_CLASS]: Sized,
{
    pub const HEADER_SIZE: usize = Cell::HEADER_BYTES;

    pub const fn new(shared: bool, base: Address) -> Self {
        debug_assert!(std::mem::size_of::<Cell>() == 32);
        Self {
            shared,
            base,
            table: [None; Config::NUM_SIZE_CLASS],
            phantom: PhantomData,
        }
    }

    fn unit_to_cell(&self, unit: Unit) -> CellPtr {
        let ptr = self.base + (*unit << Config::LOG_MIN_ALIGNMENT);
        unsafe { NonNull::new_unchecked(ptr.as_mut_ptr()) }
    }

    fn cell_to_unit(&self, cell: CellPtr) -> Unit {
        Unit((Address::from(cell.as_ptr()) - self.base) >> Config::LOG_MIN_ALIGNMENT)
    }

    pub fn pop_raw_cell(&mut self, log_size: usize) -> Option<Address> {
        let size_class =
            <Self as InternalAbstractFreeList>::size_class(self.process_input_units(1 << log_size));
        let unit = self.pop(size_class)?;
        Some(self.unit_to_value(unit))
    }
}

impl<Config: AddressSpaceConfig> IntrusiveFreeList<Config>
where
    [(); Config::NUM_SIZE_CLASS]: Sized,
{
    fn unit_to_value(&self, unit: Unit) -> Address {
        Address::from(self.unit_to_cell(unit).as_ptr())
    }

    fn value_to_unit(&self, value: Address) -> Unit {
        self.cell_to_unit(unsafe { NonNull::new_unchecked(value.as_mut_ptr()) })
    }

    const fn process_input_units(&self, units: usize) -> usize {
        units >> Config::LOG_MIN_ALIGNMENT
    }

    pub fn allocate_cell(&mut self, units: usize) -> Option<Range<Address>> {
        let units = (self.process_input_units(units) + Cell::HEADER_UNITS).next_power_of_two();
        let Range { start, end } = self.allocate_cell_aligned_size(units)?;
        let start = self.unit_to_value(start) + Cell::HEADER_BYTES;
        let end = self.unit_to_value(end);
        Some(start..end)
    }

    pub fn release_cell(&mut self, start: Address, units: usize) {
        let units = (self.process_input_units(units) + Cell::HEADER_UNITS).next_power_of_two();
        let unit = self.value_to_unit(start - Cell::HEADER_BYTES);
        self.release_cell_aligned_size(unit, units);
    }

    pub fn add_units(&mut self, start: Address, units: usize) {
        let units = self.process_input_units(units);
        debug_assert!(units.is_power_of_two());
        let unit = self.value_to_unit(start);
        self.release_cell_aligned_size(unit, units);
    }
}
