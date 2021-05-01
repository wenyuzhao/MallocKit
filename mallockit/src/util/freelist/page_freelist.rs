use std::ops::Range;
use crate::{space::page_table::PageTable, util::*};



#[derive(Debug)]
struct Cell {
    prev: Option<CellPtr>,
    next: Option<CellPtr>,
    unit: usize,
}

type CellPtr = *mut Cell;

/// Mange allocation of 0..(1 << NUM_SIZE_CLASS) units
pub struct PageFreeList<const NUM_SIZE_CLASS: usize> {
    base: Address,
    table: [Option<CellPtr>; NUM_SIZE_CLASS],
    bits: Vec<Option<Page>, System>,
    pub free_units: usize,
    pub total_units: usize,
    page_table: PageTable,
}

impl<const NUM_SIZE_CLASS: usize> PageFreeList<{NUM_SIZE_CLASS}> {
    pub fn new(base: Address) -> Self {
        Self {
            base,
            table: [None; NUM_SIZE_CLASS],
            bits: Vec::new_in(System),
            free_units: 0,
            total_units: 0,
            page_table: PageTable::new(),
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
            // debug_assert!(page.is_zeroed());
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

    #[inline]
    fn push(&mut self, unit: usize, size_class: usize) -> *mut Cell {
        let head = self.table[size_class].take();
        let mut cell = Box::leak(Box::new_in(Cell { prev: None, next: None, unit }, System));
        if let Some(mut head) = head {
            unsafe {
                debug_assert!((*head).prev.is_none());
                (*head).prev = Some(cell);
            }
        }
        cell.next = head;
        let cell_ptr = cell as *mut _;
        self.table[size_class] = Some(cell);
        cell_ptr
    }

    #[inline]
    fn pop(&mut self, size_class: usize) -> Option<usize> {
        let head = self.table[size_class].take();
        if head.is_none() {
            return None;
        } else {
            let head_ptr = head.unwrap();
            let mut head = unsafe { Box::<Cell, System>::from_raw_in(head_ptr, System) };
            let next = head.next.take();
            if let Some(next) = next {
                unsafe {
                    debug_assert_eq!((*next).prev, Some(head_ptr));
                    (*next).prev = None;
                }
            }
            self.table[size_class] = next;
            return Some(head.unit);
        }
    }

    #[inline]
    fn remove(&mut self, cell_ptr: *mut Cell, size_class: usize) {
        let mut cell = unsafe { Box::<Cell, System>::from_raw_in(cell_ptr, System) };
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
    }

    #[inline]
    fn allocate_aligned_units(&mut self, size_class: usize) -> Option<usize> {
        if size_class >= NUM_SIZE_CLASS {
            return None
        }
        if let Some(unit) = self.pop(size_class) {
            // update bitmap
            let index = Self::get_unit_index(unit, size_class);
            self.mark_index_as_allocated(index);
            self.page_table.delete_pages::<Size4K>(Page::new(self.base + (unit << 12)), 1);
            return Some(unit);
        }
        // Get a larger cell
        let super_cell = self.allocate_aligned_units(size_class + 1)?;
        debug_assert!(!self.index_is_free(Self::get_unit_index(super_cell, size_class + 1)));
        // Split into two
        let unit1 = super_cell;
        let unit2 = super_cell + (1usize << size_class);
        // Add second cell to list
        let cell_ptr = self.push(unit2, size_class);
        self.page_table.insert_pages::<Size4K>(Page::new(self.base + (unit2 << 12)), 1);
        self.page_table.set_pointer_meta(self.base + (unit2 << 12), cell_ptr.into());
        // update bitmap
        debug_assert!(!self.index_is_free(Self::get_unit_index(unit1, size_class + 1)));
        self.mark_index_as_allocated(Self::get_unit_index(unit1, size_class));
        self.mark_index_as_free(Self::get_unit_index(unit2, size_class));
        Some(unit1)
    }

    const fn index_to_unit(index: usize, size_class: usize) -> usize {
        let start = 1 << (NUM_SIZE_CLASS - size_class - 1);
        let off = index - start;
        off << size_class
    }

    #[inline]
    fn release_aligned_units(&mut self, unit: usize, size_class: usize) {
        debug_assert_eq!(unit & ((1usize << size_class) - 1), 0);
        debug_assert!(size_class < NUM_SIZE_CLASS);
        let unit_index = Self::get_unit_index(unit, size_class);
        let sibling_index = Self::get_sibling_index(unit_index);
        if unit_index > 1 && self.index_is_free(sibling_index) {
            self.mark_index_as_allocated(sibling_index);
            let parent = Self::get_parent_index(sibling_index);
            debug_assert_eq!(parent, Self::get_parent_index(unit_index));
            debug_assert!(!self.index_is_free(parent));
            // Remove sibling from list
            {
                let sibling_cell = self.page_table.get_pointer_meta(self.base + (unit << 12));
                self.remove(sibling_cell.as_mut_ptr(), size_class);
                self.page_table.delete_pages::<Size4K>(Page::new(self.base + (unit << 12)), 1);
            }
            self.release_aligned_units(Self::index_to_unit(parent, size_class), size_class + 1)
        } else {
            let cell_ptr = self.push(unit, size_class);
            self.page_table.insert_pages::<Size4K>(Page::new(self.base + (unit << 12)), 1);
            self.page_table.set_pointer_meta(self.base + (unit << 12), cell_ptr.into());
            self.mark_index_as_free(Self::get_unit_index(unit, size_class));
        }
    }

    pub const fn size_class(units: usize) -> usize {
        units.next_power_of_two().trailing_zeros() as _
    }

    /// Allocate a cell with a power-of-two size, and aligned to the size.
    #[inline]
    pub fn allocate_cell_aligned(&mut self, units: usize) -> Option<Range<usize>> {
        debug_assert!(units.is_power_of_two());
        let start = self.allocate_aligned_units(Self::size_class(units))?;
        self.free_units -= units;
        Some(start..(start + units))
    }

    /// Allocate a cell with a power-of-two alignment.
    #[inline]
    pub fn allocate_cell(&mut self, units: usize) -> Option<Range<usize>> {
        let size_class = Self::size_class(units);
        let start = self.allocate_aligned_units(size_class)?;
        let free_units = (1 << size_class) - units;
        if free_units != 0 {
            let free_start = start + units;
            self.release_cell(free_start, free_units);
        }
        self.free_units -= units;
        Some(start..(start + units))
    }

    #[inline]
    pub fn release_cell_aligned(&mut self, start: usize, units: usize) {
        debug_assert!(units.is_power_of_two());
        debug_assert!(start & (units - 1) == 0);
        self.free_units += units;
        self.release_aligned_units(start, Self::size_class(units));
    }

    #[inline]
    pub fn release_cell(&mut self, mut start: usize, mut units: usize) {
        self.free_units += units;
        let limit = start + units;
        while start < limit {
            let max_size_class = Self::size_class(units);
            for size_class in (0..=max_size_class).rev() {
                let size = 1usize << size_class;
                let end = start + size;
                if (start & (size - 1)) == 0 && end <= limit {
                    self.release_aligned_units(start, size_class);
                    start = end;
                    units = limit - end;
                    break
                }
            }
        }
        debug_assert_eq!(start, limit);
    }
}