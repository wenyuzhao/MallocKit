use std::ops::Range;
use crate::util::*;



#[derive(Debug)]
#[repr(C)]
struct Cell {
    prev: Option<CellPtr>,
    next: Option<CellPtr>,
}

type CellPtr = *mut Cell;

const MIN_SIZE_CLASS: usize = 5;

#[derive(Deref, Clone, Copy, PartialEq, Eq, Debug)]
struct Unit(usize);

impl Unit {
    fn parent(&self, size_class: usize) -> Self {
        Self(self.0 & !(1 << size_class))
    }
    fn sibling(&self, size_class: usize) -> Self {
        Self(self.0 ^ (1 << size_class))
    }
    fn is_aligned(&self, size_class: usize) -> bool {
        (self.0 & ((1usize << size_class) - 1)) == 0
    }
}

struct LazyBst {
    bits: Vec<Option<Page>, System>,
}

impl LazyBst {
    const fn new() -> Self {
        Self { bits: Vec::new_in(System) }
    }
    fn resize(&mut self, index: BstIndex) {
        let index = *index;
        let byte_index = index >> 3;
        let page_index = byte_index >> Size2M::LOG_BYTES;
        if page_index >= self.bits.len() {
            self.bits.resize((page_index + 1).next_power_of_two(), None);
        }
        if self.bits[page_index].is_none() {
            let page = unsafe {
                let addr = libc::mmap(0 as _, Size2M::BYTES, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS, -1, 0);
                let addr = Address::from(addr);
                Page::new(addr)
            };
            debug_assert!(page.is_zeroed());
            self.bits[page_index] = Some(page);
        }
    }
    fn get_bit_location(&self, index: BstIndex) -> Option<(Address, usize)> {
        let index = *index;
        let byte_index = index >> 3;
        let page_index = byte_index >> Size2M::LOG_BYTES;
        if page_index >= self.bits.len() { return None }
        let byte_offset_in_page = byte_index & Page::<Size2M>::MASK;
        let bit_offset_in_byte = index & 0b111;
        let addr = self.bits[page_index]?.start() + byte_offset_in_page;
        Some((addr, bit_offset_in_byte))
    }
    fn get(&self, index: BstIndex) -> Option<bool> {
        let (addr, bit_index) = self.get_bit_location(index)?;
        Some(unsafe { (addr.load::<u8>() & (1 << bit_index)) != 0 })
    }
    fn set(&mut self, index: BstIndex, value: bool) {
        self.resize(index);
        let (addr, bit_index) = self.get_bit_location(index).unwrap();
        if value {
            unsafe { addr.store::<u8>(addr.load::<u8>() | (1 << bit_index)); }
        } else {
            unsafe { addr.store::<u8>(addr.load::<u8>() & !(1 << bit_index)); }
        }
    }
}

#[derive(Deref, Clone, Copy, PartialEq, Eq, Debug)]
struct BstIndex(usize);

/// Manage allocation of 0..(1 << NUM_SIZE_CLASS) units
pub struct FreeList<const NUM_SIZE_CLASS: usize> {
    base: Address,
    table: [Option<CellPtr>; NUM_SIZE_CLASS],
    bst: LazyBst,
    pub free_units: usize,
    pub total_units: usize,
}

impl<const NUM_SIZE_CLASS: usize> FreeList<{NUM_SIZE_CLASS}> {
    pub const fn new(base: Address) -> Self {
        Self {
            base,
            table: [None; NUM_SIZE_CLASS],
            bst: LazyBst::new(),
            free_units: 0,
            total_units: 0,
        }
    }

    fn unit_to_index(&self, unit: Unit, size_class: usize) -> BstIndex {
        let start = 1 << (NUM_SIZE_CLASS - size_class - 1);
        let index = *unit >> size_class;
        debug_assert!(start + index < (1 << (NUM_SIZE_CLASS - size_class)));
        BstIndex(start + index)
    }

    fn unit_to_cell(&self, unit: Unit) -> *mut Cell {
        (self.base + *unit).as_mut_ptr()
    }

    fn cell_to_unit(&self, cell: *mut Cell) -> Unit {
        Unit(Address::from(cell) - self.base)
    }

    fn is_free(&self, unit: Unit, size_class: usize) -> bool {
        self.bst.get(self.unit_to_index(unit, size_class)).unwrap_or(false)
    }

    fn set_as_free(&mut self, unit: Unit, size_class: usize) {
        if cfg!(feature="slow_assert") {
            debug_assert!(self.is_not_free_slow(unit));
        }
        self.bst.set(self.unit_to_index(unit, size_class), true);
    }

    fn set_as_used(&mut self, unit: Unit, size_class: usize) {
        debug_assert!(self.is_free(unit, size_class));
        self.bst.set(self.unit_to_index(unit, size_class), false);
        if cfg!(feature="slow_assert") {
            debug_assert!(self.is_not_free_slow(unit));
        }
    }

    fn push(&mut self, unit: Unit, size_class: usize) -> *mut Cell {
        if cfg!(feature="slow_assert") {
            debug_assert!(!self.is_on_current_list_slow(unit, None));
        }
        let head = self.table[size_class].take();
        let mut cell = unsafe { &mut *self.unit_to_cell(unit) };
        cell.prev = None;
        if let Some(mut head) = head {
            unsafe {
                debug_assert!((*head).prev.is_none());
                (*head).prev = Some(cell);
            }
        }
        cell.next = head;
        let cell_ptr = cell as *mut _;
        self.table[size_class] = Some(cell);
        debug_assert!(self.cell_to_unit(cell_ptr) == unit);
        self.set_as_free(unit, size_class);
        if cfg!(feature="slow_assert") {
            debug_assert!(self.is_on_current_list_slow(unit, Some(size_class)));
        }
        cell_ptr
    }

    fn pop(&mut self, size_class: usize) -> Option<*mut Cell> {
        let head = self.table[size_class].take();
        if head.is_none() {
            return None;
        } else {
            let head_ptr = head.unwrap();
            let head = unsafe { &mut *head_ptr };
            let next = head.next.take();
            if let Some(next) = next {
                unsafe {
                    debug_assert_eq!((*next).prev, Some(head_ptr));
                    (*next).prev = None;
                }
            }
            self.table[size_class] = next;
            debug_assert!(head.prev.is_none());
            debug_assert!(head.next.is_none());
            self.set_as_used(self.cell_to_unit(head), size_class);
            return Some(head);
        }
    }

    fn remove(&mut self, cell_ptr: *mut Cell, size_class: usize) {
        let cell = unsafe { &mut *cell_ptr };
        let next = cell.next.take();
        let prev = cell.prev.take();
        if let Some(prev) = prev {
            unsafe {
                debug_assert_eq!((*prev).next, Some(cell_ptr));
                (*prev).next = next;
            }
        } else {
            debug_assert_eq!(self.table[size_class], Some(cell_ptr));
            self.table[size_class] = next;
        }
        if let Some(next) = next {
            unsafe {
                debug_assert_eq!((*next).prev, Some(cell_ptr));
                (*next).prev = prev;
            }
        }
        self.set_as_used(self.cell_to_unit(cell_ptr), size_class);
    }

    fn allocate_aligned_units(&mut self, size_class: usize) -> Option<Unit> {
        if size_class >= NUM_SIZE_CLASS {
            return None
        }
        if let Some(cell) = self.pop(size_class) {
            let unit = self.cell_to_unit(cell);
            debug_assert!(!self.is_free(unit, size_class));
            return Some(unit);
        }
        // Get a larger cell
        let parent = self.allocate_aligned_units(size_class + 1)?;
        debug_assert!(!self.is_free(parent, size_class + 1)); // parent is used
        // Split into two
        let unit1 = parent;
        let unit2 = unit1.sibling(size_class);
        debug_assert!(!self.is_free(unit1, size_class)); // child#0 is used
        debug_assert!(!self.is_free(unit2, size_class)); // child#1 is used
        // Add second cell to list
        debug_assert!(size_class < NUM_SIZE_CLASS);
        self.push(unit2, size_class);
        debug_assert!(!self.is_free(parent, size_class + 1)); // parent is used
        debug_assert!(!self.is_free(unit1, size_class)); // child#0 is used
        debug_assert!(self.is_free(unit2, size_class)); // child#1 is free
        Some(unit1)
    }

    fn is_not_free_slow(&self, unit: Unit) -> bool {
        assert!(cfg!(feature="slow_assert"));
        for sz in 0..NUM_SIZE_CLASS {
            if self.is_free(unit, sz) { return true }
        }
        false
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
                    head = (*c).next;
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

    fn release_aligned_units(&mut self, unit: Unit, size_class: usize) {
        debug_assert!(unit.is_aligned(size_class));
        debug_assert!(size_class < NUM_SIZE_CLASS);
        let sibling = unit.sibling(size_class);
        debug_assert!(!self.is_free(unit, size_class));
        if (size_class < NUM_SIZE_CLASS - 1) && self.is_free(sibling, size_class) {
            if cfg!(feature="slow_assert") {
                debug_assert!(self.is_on_current_list_slow(sibling, Some(size_class)));
            }
            let parent = unit.parent(size_class);
            debug_assert!(!self.is_free(parent, size_class + 1), "{:?} {}", parent, size_class); // parent is used
            // Remove sibling from list
            self.remove(self.unit_to_cell(sibling), size_class);
            debug_assert!(!self.is_free(unit, size_class)); // unit is used
            debug_assert!(!self.is_free(sibling, size_class)); // sibling is used
            // Merge unit and sibling
            self.release_aligned_units(parent, size_class + 1);
        } else {
            debug_assert!(size_class < NUM_SIZE_CLASS);
            if cfg!(feature="slow_assert") {
                debug_assert!(!self.is_on_current_list_slow(unit, None));
            }
            self.push(unit, size_class);
            debug_assert!(self.is_free(unit, size_class)); // unit is free
            if cfg!(feature="slow_assert") {
                debug_assert!(self.is_on_current_list_slow(unit, Some(size_class)));
            }
        }
    }

    pub const fn size_class(units: usize) -> usize {
        let a = units.next_power_of_two().trailing_zeros() as _;
        let b = MIN_SIZE_CLASS;
        if a > b { a } else { b }
    }

    /// Allocate a cell with a power-of-two size, and aligned to the size.
    #[inline]
    pub fn allocate_cell_aligned(&mut self, units: usize) -> Option<Range<usize>> {
        debug_assert!(units.is_power_of_two());
        let size_class = Self::size_class(units);
        let start = self.allocate_aligned_units(size_class)?;
        // debug_assert!(!self.index_is_free(Self::get_unit_index(start, Self::size_class(units))));
        self.free_units -= units;
        Some(*start..(*start + units))
    }

    #[inline]
    pub fn release_cell_aligned(&mut self, start: usize, units: usize) {
        debug_assert!(units.is_power_of_two());
        debug_assert!(start & (units - 1) == 0);
        self.free_units += units;
        let size_class = Self::size_class(units);
        self.release_aligned_units(Unit(start), size_class);
    }

    // /// Allocate a cell with a power-of-two alignment.
    // #[inline]
    // pub fn allocate_cell(&mut self, units: usize) -> Option<Range<usize>> {
    //     unreachable!();
    //     let size_class = Self::size_class(units);
    //     let start = self.allocate_aligned_units(size_class)?;

    //     let free_units = (1 << size_class) - units;
    //     if free_units != 0 {
    //         let free_start = start + units;
    //         self.release_cell(free_start, free_units);
    //     }
    //     self.free_units -= units;
    //     Some(start..(start + units))
    // }

    // #[inline]
    // pub fn release_cell(&mut self, mut start: usize, mut units: usize) {
    //     unreachable!();
    //     self.free_units += units;
    //     let limit = start + units;
    //     while start < limit {
    //         let max_size_class = Self::size_class(units);
    //         for size_class in (0..=max_size_class).rev() {
    //             let size = 1usize << size_class;
    //             let end = start + size;
    //             if (start & (size - 1)) == 0 && end <= limit {
    //                 self.release_aligned_units(start, size_class, false);
    //                 start = end;
    //                 units = limit - end;
    //                 break
    //             }
    //         }
    //     }
    //     debug_assert_eq!(start, limit);
    // }
}