use std::{ops::Range, sync::atomic::{AtomicUsize, Ordering}};
use std::iter::Step;
use crate::util::*;
use spin::Mutex;
use super::{PAGE_REGISTRY, SpaceId};

#[derive(Debug)]
struct Cell {
    next: Option<Box<Cell, System>>,
    unit: usize,
}

const NUM_SIZE_CLASS: usize = SpaceId::LOG_MAX_SPACE_SIZE - Page::<Size4K>::LOG_BYTES + 1;
// const LOG_PAGE_SIZE: usize = 12;

pub struct PageResource {
    pub id: SpaceId,
    base: Address,
    freelist: Mutex<[Option<Box<Cell, System>>; NUM_SIZE_CLASS]>,
    committed_size: AtomicUsize,
}

impl PageResource {
    pub fn new(id: SpaceId) -> Self {
        debug_assert!(id.0 < 0b0000_1111);
        let base = Address::from(1usize << 45) + ((id.0 as usize) << 41);
        let pr = Self {
            id,
            base,
            freelist: Mutex::new(array_init::array_init(|_| None)),
            committed_size: AtomicUsize::new(0),
        };
        pr.freelist.lock()[NUM_SIZE_CLASS - 1] = Some(Box::new_in(Cell { next: None, unit: 0 }, System));
        pr
    }

    #[inline(always)]
    pub fn committed_size(&self) -> usize {
        self.committed_size.load(Ordering::SeqCst)
    }

    fn pages_to_size_class<S: PageSize>(pages: usize) -> usize {
        let small_pages =pages << (S::LOG_BYTES - Size4K::LOG_BYTES);
        small_pages.next_power_of_two().trailing_zeros() as _
    }

    fn address_to_unit(&self, address: Address) -> usize {
        (address - self.base) >> Page::<Size4K>::LOG_BYTES
    }

    fn unit_to_address(&self, unit: usize) -> Address {
        self.base + (unit << Page::<Size4K>::LOG_BYTES)
    }

    fn try_allocate_unit(freelist: &mut [Option<Box<Cell, System>>; NUM_SIZE_CLASS], size_class: usize) -> Option<usize> {
        if size_class >= NUM_SIZE_CLASS {
            return None
        }
        match freelist[size_class].take() {
            Some(mut cell) => {
                debug_assert!(freelist[size_class].is_none());
                freelist[size_class] = cell.next.take();
                Some(cell.unit)
            }
            None => {
                let super_cell = Self::try_allocate_unit(freelist, size_class + 1)?;
                let unit1 = super_cell;
                let unit2 = super_cell + (1usize << size_class);
                debug_assert!(freelist[size_class].is_none());
                freelist[size_class] = Some(Box::new_in(Cell { next: None, unit: unit2 }, System));
                Some(unit1)
            }
        }
    }

    fn release_unit(freelist: &mut [Option<Box<Cell, System>>; NUM_SIZE_CLASS], unit: usize, size_class: usize) {
        debug_assert_eq!(unit & ((1usize << size_class) - 1), 0);
        debug_assert!(size_class < NUM_SIZE_CLASS);
        // Get sibling of `unit`
        let unit2 = if (unit & (1 << size_class)) == 0 {
            unit + (1 << size_class)
        } else {
            unit & !((1 << size_class))
        };
        let sibling_in_freelist = {
            let mut found = false;
            let mut head = &mut freelist[size_class];
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
            Self::release_unit(freelist, usize::min(unit, unit2), size_class + 1)
        } else {
            let head = freelist[size_class].take();
            freelist[size_class] = Some(Box::new_in(Cell { next: head, unit }, System));
        }
    }

    fn map_pages<S: PageSize>(&self, start: Page<S>, pages: usize) -> bool {
        let addr = unsafe { libc::mmap(start.start().as_mut_ptr(), pages << S::LOG_BYTES, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED_NOREPLACE, -1, 0) };
        if addr == libc::MAP_FAILED {
            false
        } else {
            self.committed_size.fetch_add(pages << S::LOG_BYTES, Ordering::SeqCst);
            true
        }
    }

    fn unmap_pages<S: PageSize>(&self, start: Page<S>, pages: usize) {
        unsafe { libc::munmap(start.start().as_mut_ptr(), pages << S::LOG_BYTES); }
        self.committed_size.fetch_sub(pages << S::LOG_BYTES, Ordering::SeqCst);
    }

    fn try_acquire_pages_locked<S: PageSize>(&self, freelist: &mut [Option<Box<Cell, System>>; NUM_SIZE_CLASS], pages: usize) -> Option<Range<Page<S>>> {
        debug_assert_ne!(pages, 0);
        let size_class = Self::pages_to_size_class::<S>(pages);
        let actural_pages = 1usize << size_class;
        let unit = Self::try_allocate_unit(freelist, size_class)?;
        let start = self.unit_to_address(unit);
        debug_assert!(Page::<S>::is_aligned(start));
        let start = Page::<S>::new(start);
        if !self.map_pages(start, actural_pages) {
            return self.try_acquire_pages_locked(freelist, pages); // Retry
        }
        let end = Step::forward(start, actural_pages);
        PAGE_REGISTRY.insert_pages(start, actural_pages);
        Some(start..end)
    }

    pub fn acquire_pages<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        let mut freelist = self.freelist.lock();
        self.try_acquire_pages_locked(&mut freelist, pages)
    }

    pub fn release_pages<S: PageSize>(&self, start: Page<S>) {
        let pages = PAGE_REGISTRY.delete_pages(start);
        self.unmap_pages(start, pages);
        let size_class = Self::pages_to_size_class::<S>(pages);
        let unit = self.address_to_unit(start.start());
        let mut freelist = self.freelist.lock();
        Self::release_unit(&mut freelist, unit, size_class)
    }
}