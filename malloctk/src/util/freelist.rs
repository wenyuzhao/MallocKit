use std::ops::Range;
use crate::util::*;



#[derive(Debug)]
struct Cell {
    next: Option<Box<Cell, System>>,
    unit: usize,
}


pub struct FreeList<const NUM_SIZE_CLASS: usize> {
    table: [Option<Box<Cell, System>>; NUM_SIZE_CLASS],
    pub free_units: usize,
    pub total_units: usize,
}

impl<const NUM_SIZE_CLASS: usize> FreeList<{NUM_SIZE_CLASS}> {
    pub fn new() -> Self {
        Self {
            table: array_init::array_init(|_| None),
            free_units: 0,
            total_units: 0,
        }
    }

    fn units_to_size_class(units: usize) -> usize {
        units.next_power_of_two().trailing_zeros() as _
    }

    fn allocate_aligned_units(&mut self, size_class: usize) -> Option<usize> {
        if size_class >= NUM_SIZE_CLASS {
            return None
        }
        match self.table[size_class].take() {
            Some(mut cell) => {
                debug_assert!(self.table[size_class].is_none());
                self.table[size_class] = cell.next.take();
                Some(cell.unit)
            }
            None => {
                let super_cell = self.allocate_aligned_units(size_class + 1)?;
                let unit1 = super_cell;
                let unit2 = super_cell + (1usize << size_class);
                debug_assert!(self.table[size_class].is_none());
                self.table[size_class] = Some(Box::new_in(Cell { next: None, unit: unit2 }, System));
                Some(unit1)
            }
        }
    }

    pub fn allocate(&mut self, units: usize) -> Option<Range<usize>> {
        debug_assert!(units.is_power_of_two());
        let size_class = Self::units_to_size_class(units);
        let units = 1 << size_class;
        let start = self.allocate_aligned_units(size_class)?;
        self.free_units -= units;
        Some(start..(start + units))
    }

    fn release_aligned_units(&mut self, unit: usize, size_class: usize) {
        debug_assert_eq!(unit & ((1usize << size_class) - 1), 0);
        debug_assert!(size_class < NUM_SIZE_CLASS);
        // Get sibling of `unit`
        let unit2 = if (unit & (1 << size_class)) == 0 {
            unit + (1 << size_class)
        } else {
            unit & !((1 << size_class))
        };
        let is_max_size_class = size_class + 1 == NUM_SIZE_CLASS;
        let sibling_in_freelist = !is_max_size_class && {
            let mut found = false;
            let mut head = &mut self.table[size_class];
            while head.is_some() {
                if head.as_ref().map(|x| x.unit).unwrap() == unit2 {
                    // Remove sibling from freelist
                    let next = head.as_mut().unwrap().next.take();
                    *head = next;
                    found = true;
                    break;
                }
                head =  &mut head.as_mut().unwrap().next;
            }
            found
        };
        if sibling_in_freelist {
            self.release_aligned_units(usize::min(unit, unit2), size_class + 1)
        } else {
            let head = self.table[size_class].take();
            self.table[size_class] = Some(Box::new_in(Cell { next: head, unit }, System));
        }
    }

    pub fn release(&mut self, start: usize, units: usize) {
        debug_assert!(units.is_power_of_two());
        let size_class = Self::units_to_size_class(units);
        let units = 1 << size_class;
        self.free_units += units;
        self.release_aligned_units(start, size_class);
    }
}