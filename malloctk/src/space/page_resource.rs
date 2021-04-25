use crate::util::{Address, System};
use spin::Mutex;
use super::{PAGE_REGISTRY, SpaceId};

#[derive(Debug)]
struct Cell {
    next: Option<Box<Cell, System>>,
    unit: usize,
}

const NUM_SIZE_CLASS: usize = SpaceId::LOG_MAX_SPACE_SIZE - 12 + 1;
const LOG_PAGE_SIZE: usize = 12;

pub struct PageResource {
    pub id: SpaceId,
    base: Address,
    freelist: Mutex<[Option<Box<Cell, System>>; NUM_SIZE_CLASS]>,
}

impl PageResource {
    pub fn new(id: SpaceId) -> Self {
        debug_assert!(id.0 < 0b0000_1111);
        let base = Address::from(1usize << 45) + ((id.0 as usize) << 41);
        let pr = Self {
            id,
            base,
            freelist: Mutex::new(array_init::array_init(|_| None)),
        };
        pr.freelist.lock()[NUM_SIZE_CLASS - 1] = Some(Box::new_in(Cell { next: None, unit: 0 }, System));
        pr
    }

    fn pages_to_size_class(pages: usize) -> usize {
        pages.next_power_of_two().trailing_zeros() as _
    }

    fn address_to_unit(&self, address: Address) -> usize {
        (address - self.base) >> LOG_PAGE_SIZE
    }

    fn unit_to_address(&self, unit: usize) -> Address {
        self.base + (unit << LOG_PAGE_SIZE)
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
        // TODO: Merge cells
        let head = freelist[size_class].take();
        freelist[size_class] = Some(Box::new_in(Cell { next: head, unit }, System));
    }

    unsafe fn map_pages(start: Address, pages: usize) -> bool {
        let addr = libc::mmap(start.as_mut_ptr(), pages << LOG_PAGE_SIZE, libc::PROT_READ | libc::PROT_WRITE, libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_FIXED_NOREPLACE, -1, 0);
        if addr == libc::MAP_FAILED {
            false
        } else {
            true
        }
    }

    unsafe fn unmap_pages(start: Address, pages: usize) {
        libc::munmap(start.as_mut_ptr(), pages << LOG_PAGE_SIZE);
    }

    fn try_acquire_pages_locked(&self, freelist: &mut [Option<Box<Cell, System>>; NUM_SIZE_CLASS], pages: usize) -> Option<Address> {
        debug_assert_ne!(pages, 0);
        let size_class = Self::pages_to_size_class(pages);
        let actural_pages = 1usize << size_class;
        let unit = Self::try_allocate_unit(freelist, size_class)?;
        let start = self.unit_to_address(unit);
        if unsafe { !Self::map_pages(start, actural_pages) } {
            return self.try_acquire_pages_locked(freelist, pages); // Retry
        }
        PAGE_REGISTRY.insert_pages(start, actural_pages);
        Some(start)
    }

    pub fn acquire_pages(&self, pages: usize) -> Option<Address> {
        let mut freelist = self.freelist.lock();
        self.try_acquire_pages_locked(&mut freelist, pages)
    }

    pub fn release_pages(&self, start: Address) {
        let pages = PAGE_REGISTRY.get_contiguous_pages(start);
        PAGE_REGISTRY.delete_pages(start, pages);
        unsafe { Self::unmap_pages(start, pages) };
        let size_class = Self::pages_to_size_class(pages);
        let unit = self.address_to_unit(start);
        let mut freelist = self.freelist.lock();
        Self::release_unit(&mut freelist, unit, size_class)
    }
}