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

/// Manage allocation of 0..(1 << NUM_SIZE_CLASS) units
pub struct FreeList<const NUM_SIZE_CLASS: usize> {
    base: Address,
    table: [Option<CellPtr>; NUM_SIZE_CLASS],
    bits: Vec<Option<Page>, System>,
    pub free_units: usize,
    pub total_units: usize,
}

#[derive(Deref, Clone, Copy, PartialEq, Eq)]
#[deref(forward)]
struct Unit(usize);

#[derive(Deref, Clone, Copy, PartialEq, Eq)]
#[deref(forward)]
struct BstIndex(usize);

impl<const NUM_SIZE_CLASS: usize> FreeList<{NUM_SIZE_CLASS}> {
    pub const fn new(base: Address) -> Self {
        Self {
            base,
            table: [None; NUM_SIZE_CLASS],
            bits: Vec::new_in(System),
            free_units: 0,
            total_units: 0,
        }
    }

    const fn get_parent_index(index: usize) -> usize {
        index >> 1
    }

    const fn get_sibling_index(index: usize) -> usize {
        index ^ 1
    }

    fn resize(&mut self, index: usize) {
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

    fn get_bit_location(&mut self, index: usize) -> (Address, usize) {
        self.resize(index);
        let byte_index = index >> 3;
        let page_index = byte_index >> Size2M::LOG_BYTES;
        let byte_offset_in_page = byte_index & Page::<Size2M>::MASK;
        let bit_offset_in_byte = index & 0b111;
        let addr = self.bits[page_index].unwrap().start() + byte_offset_in_page;
        (addr, bit_offset_in_byte)
    }

    fn index_is_free(&mut self, index: usize) -> bool {
        let (addr, bit_index) = self.get_bit_location(index);
        unsafe { (addr.load::<u8>() & (1 << bit_index)) == 1 }
    }

    fn mark_index_as_free(&mut self, index: usize) {
        let (addr, bit_index) = self.get_bit_location(index);
        unsafe { addr.store::<u8>(addr.load::<u8>() | (1 << bit_index)); }
    }

    fn mark_index_as_allocated(&mut self, index: usize) {
        self.resize(index);
        let (addr, bit_index) = self.get_bit_location(index);
        unsafe {
            addr.store::<u8>(addr.load::<u8>() & !(1 << bit_index));
        }
    }

    const fn get_unit_index(unit: usize, size_class: usize) -> usize {
        let start = 1 << (NUM_SIZE_CLASS - size_class - 1);
        let index = unit >> size_class;
        start + index
    }

    const fn unit_to_cell(&self, unit: usize) -> *mut Cell {
        (self.base + unit).as_mut_ptr()
    }

    const fn cell_to_unit(&self, cell: *mut Cell) -> usize {
        Address::from(cell) - self.base
    }

    const fn index_to_unit(&self, index: usize, size_class: usize) -> usize {
        let start = 1 << (NUM_SIZE_CLASS - size_class - 1);
        let off = index - start;
        let x = off << size_class;
        debug_assert!(Self::get_unit_index(x, size_class) == index);
        x
    }

    #[inline]
    fn push(&mut self, unit: usize, size_class: usize) -> *mut Cell {
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
        cell_ptr
    }

    #[inline]
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
            return Some(head);
        }
    }

    #[inline]
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
            debug_assert_eq!(self.table[size_class], Some(cell_ptr), "{:?} list@{:?}", self.cell_to_unit(cell_ptr), self as *const _);
            // if self.table[size_class] == Some(cell_ptr) {
                self.table[size_class] = next;
            // } else {
                // self.table[size_class] = next;
            // }

        }
        if let Some(next) = next {
            unsafe {
                debug_assert_eq!((*next).prev, Some(cell_ptr));
                (*next).prev = prev;
            }
        }
    }

    #[inline]
    fn allocate_aligned_units(&mut self, size_class: usize) -> Option<usize> {
        if size_class >= NUM_SIZE_CLASS {
            return None
        }
        if let Some(cell) = self.pop(size_class) {
            let unit = self.cell_to_unit(cell);
            // update bitmap
            let index = Self::get_unit_index(unit, size_class);
            self.mark_index_as_allocated(index);
            debug_assert!(!self.index_is_free(index));
            return Some(unit);
        }
        // Get a larger cell
        let super_cell = self.allocate_aligned_units(size_class + 1)?;
        debug_assert!(!self.index_is_free(Self::get_unit_index(super_cell, size_class + 1)));
        // Split into two
        let unit1 = super_cell;
        let unit2 = super_cell + (1usize << size_class);
        debug_assert!(!self.index_is_free(Self::get_unit_index(unit1, size_class)));
        debug_assert!(!self.index_is_free(Self::get_unit_index(unit2, size_class)));
        // Add second cell to list
        debug_assert!(size_class < NUM_SIZE_CLASS);
        self.push(unit2, size_class);
        // update bitmap
        // debug_assert!(!self.index_is_free(Self::get_unit_index(unit1, size_class + 1)));
        self.mark_index_as_allocated(Self::get_unit_index(unit1, size_class));
        // debug_assert!(!self.index_is_free(Self::get_unit_index(0x3ca000, 10)));
        self.mark_index_as_free(Self::get_unit_index(unit2, size_class));
        // debug_assert!(!self.index_is_free(Self::get_unit_index(unit1, size_class)), "{:x} {}", unit1, size_class);
        Some(unit1)
    }

    #[inline]
    fn is_on_current_list_slow(&mut self, unit: usize, size_class: usize) -> bool {
        let mut head = self.table[size_class];
        while let Some(c) = head {
            unsafe {
                if self.cell_to_unit(c) == unit {
                    return true;
                }
                head = (*c).next;
            }
        }
        false
    }

    #[inline(never)]
    fn release_aligned_units(&mut self, unit: usize, size_class: usize) {
        debug_assert_eq!(unit & ((1usize << size_class) - 1), 0);
        debug_assert!(size_class < NUM_SIZE_CLASS);
        let unit_index = Self::get_unit_index(unit, size_class);
        let sibling_index = Self::get_sibling_index(unit_index);
        debug_assert!(!self.index_is_free(unit_index));
        if (size_class < NUM_SIZE_CLASS - 1) && (unit_index > 1) && self.index_is_free(sibling_index) {
            debug_assert!(self.is_on_current_list_slow(unit ^ (1 << size_class), size_class), "self.index_is_free(sibling_index)={} sibling_index={} sibling_unit={} index={} unit={:?} list={:?} size_class={}", self.index_is_free(sibling_index), sibling_index, self.index_to_unit(sibling_index, size_class), unit_index, unit, self as *const _, size_class);
            self.mark_index_as_allocated(sibling_index);
            let parent = Self::get_parent_index(sibling_index);
            debug_assert_eq!(parent, Self::get_parent_index(unit_index));
            debug_assert!(!self.index_is_free(parent));
            // Remove sibling from list
            {
                // debug_assert!(self.is_on_current_list_slow(self.index_to_unit(sibling_index, size_class), size_class));
                let sibling_cell = self.unit_to_cell(self.index_to_unit(sibling_index, size_class));
                self.remove(sibling_cell, size_class);
            }
            let sibling_unit = self.index_to_unit(sibling_index, size_class);
            let parent_unit = self.index_to_unit(parent, size_class + 1);
            debug_assert!(parent_unit == sibling_unit || parent_unit == unit);
            // if size_class + 1 >= NUM_SIZE_CLASS {
            self.release_aligned_units(parent_unit, size_class + 1)
            // }
        } else {
            self.mark_index_as_free(Self::get_unit_index(unit, size_class));
            debug_assert!(size_class < NUM_SIZE_CLASS);
            self.push(unit, size_class);
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
        Some(start..(start + units))
    }

    #[inline]
    pub fn release_cell_aligned(&mut self, start: usize, units: usize) {
        debug_assert!(units.is_power_of_two());
        debug_assert!(start & (units - 1) == 0);
        self.free_units += units;
        let size_class = Self::size_class(units);
        self.release_aligned_units(start, size_class);
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