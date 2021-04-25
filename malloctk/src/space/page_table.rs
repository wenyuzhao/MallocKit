use spin::RwLock;
use crate::util::{Address, System};
use std::{marker::PhantomData, mem, sync::atomic::{AtomicUsize, Ordering}};



struct BitField { bits: usize, shift: usize }

impl BitField {
    const fn get(&self, value: usize) -> usize {
        (value >> self.shift) & ((1usize << self.bits) - 1)
    }

    const fn set(&self, slot: &mut usize, value: usize) {
        let old_value = *slot;
        let mask = ((1usize << self.bits) - 1) << self.shift;
        let shifted_value = value << self.shift;
        debug_assert!((shifted_value & !mask) == 0);
        let new_value = (old_value & !mask) | (value << self.shift);
        *slot = new_value;
    }

    const fn delta(&self, slot: &mut usize, delta: isize) -> usize {
        let old = self.get(*slot);
        let new = if delta > 0 { old + (delta as usize) } else { old - ((-delta) as usize) };
        self.set(slot, new);
        new
    }
}

#[repr(transparent)]
struct PageTableEntry<L: PageTableLevel>(usize, PhantomData<L>);

enum PageTableEntryData<L: PageTableLevel> {
    NextLevelPageTable { table: &'static mut PageTable<L> },
    Page4K { contiguous_pages: Option<usize> },
}

impl<L: PageTableLevel> PageTableEntry<L> {
    // Global fields
    const PRESENT: BitField = BitField { bits: 1, shift: 63 };
    const IS_PAGE_TABLE: BitField = BitField { bits: 1, shift: 62 };
    // Page table fields
    const PAGE_TABLE_POINTER_MASK: usize = 0x0000_ffff_ffff_f000; // 1: page table, 0: page
    const PAGE_TABLE_USED_ENTRIES: BitField = BitField { bits: 8, shift: 0 };
    // Page fields
    const PAGE_CONTIGUOUS_PAGES: BitField = BitField { bits: 16, shift: 8 };

    fn clear(&mut self) {
        let value = self.0;
        if Self::PRESENT.get(value) != 0 && Self::IS_PAGE_TABLE.get(value) != 0 {
            let table: &'static mut PageTable<L::NextLevel> = unsafe { mem::transmute(value & Self::PAGE_TABLE_POINTER_MASK) };
            let _ = unsafe { Box::from_raw_in(table, System) };
        }
        self.0 = 0;
    }

    const fn get(&self) -> Option<PageTableEntryData<L::NextLevel>> {
        let value = self.0;
        if Self::PRESENT.get(value) == 0 { return None }
        if Self::IS_PAGE_TABLE.get(value) != 0 {
            let table: &'static mut PageTable<L::NextLevel> = unsafe { mem::transmute(value & Self::PAGE_TABLE_POINTER_MASK) };
            Some(PageTableEntryData::NextLevelPageTable { table })
        } else {
            let contiguous_pages = Self::PAGE_CONTIGUOUS_PAGES.get(value);
            Some(PageTableEntryData::Page4K { contiguous_pages: if contiguous_pages == 0 { None } else { Some(contiguous_pages) } })
        }
    }

    fn set_next_page_table(&mut self, table: &'static mut PageTable<L::NextLevel>) {
        debug_assert!(self.0 == 0);
        Self::PRESENT.set(&mut self.0, 1);
        Self::IS_PAGE_TABLE.set(&mut self.0, 1);
        Self::PAGE_TABLE_USED_ENTRIES.set(&mut self.0, 0);
        let word = table as *mut _ as usize;
        debug_assert!((word & !Self::PAGE_TABLE_POINTER_MASK) == 0);
        self.0 = self.0 | word;
    }

    fn set_next_page(&mut self, pages: Option<usize>) {
        debug_assert!(self.0 == 0);
        Self::PRESENT.set(&mut self.0, 1);
        Self::IS_PAGE_TABLE.set(&mut self.0, 0);
        Self::PAGE_CONTIGUOUS_PAGES.set(&mut self.0, pages.unwrap_or(0));
    }

    const fn delta_entries(&mut self, entries: i32) -> usize {
        Self::PAGE_TABLE_USED_ENTRIES.delta(&mut self.0, entries as _)
    }
}

trait PageTableLevel: 'static {
    type NextLevel: PageTableLevel;
    const SHIFT: usize = Self::NextLevel::SHIFT + 9;
    const MASK: usize = 0b1_1111_1111 << Self::SHIFT;

    #[inline(always)]
    fn get_index(addr: Address) -> usize {
        (usize::from(addr) & Self::MASK) >> Self::SHIFT
    }
}

pub struct L4;

impl PageTableLevel for L4 {
    type NextLevel = L3;
}

struct L3;

impl PageTableLevel for L3 {
    type NextLevel = L2;
}

struct L2;

impl PageTableLevel for L2 {
    type NextLevel = L1;
}

struct L1;

impl PageTableLevel for L1 {
    type NextLevel = !;
    const SHIFT: usize = 12;
}

impl PageTableLevel for ! {
    type NextLevel = !;
    const SHIFT: usize = 0;
}

struct PageMeta {
    pub contiguous_pages: Option<usize>,
}

struct PageTable<L: PageTableLevel> {
    table: [PageTableEntry::<L>; 512],
    phantom: PhantomData<L>,
}

impl<L: PageTableLevel> PageTable<L> {
    #[inline(always)]
    fn get_entry(&self, address: Address) -> Option<PageTableEntryData<L::NextLevel>> {
        self.table[L::get_index(address)].get()
    }

    fn get_next_page_table(&self, address: Address) -> &'static mut PageTable::<L::NextLevel> {
        match self.table[L::get_index(address)].get() {
            Some(PageTableEntryData::NextLevelPageTable { table, .. }) => table,
            _ => unreachable!(),
        }
    }

    fn get_or_allocate_next_page_table(&mut self, address: Address, mut on_create: impl FnMut()) -> &'static mut PageTable::<L::NextLevel> {
        let index = L::get_index(address);
        match self.table[index].get() {
            Some(PageTableEntryData::NextLevelPageTable { table, .. }) => table,
            Some(_) => unreachable!(),
            _ => {
                let table = Box::leak(Box::new_in(PageTable::<L::NextLevel> {
                    table: unsafe { mem::transmute([0usize; 512]) },
                    phantom: PhantomData
                }, System));
                self.table[index].set_next_page_table(table);
                on_create();
                self.get_next_page_table(address)
            }
        }
    }
}

impl PageTable<L4> {
    const fn new() -> Self {
        Self {
            table: unsafe { mem::transmute([0usize; 512]) },
            phantom: PhantomData
        }
    }

    #[inline(always)]
    fn get(&self, address: Address) -> Option<PageMeta> {
        let l3 = match self.get_entry(address)? {
            PageTableEntryData::NextLevelPageTable { table, .. } => table,
            _ => unreachable!(),
        };
        let l2 = match l3.get_entry(address)? {
            PageTableEntryData::NextLevelPageTable { table, .. } => table,
            _ => unreachable!(), // 1G page
        };
        let l1 = match l2.get_entry(address)? {
            PageTableEntryData::NextLevelPageTable { table, .. } => table,
            _ => unreachable!(), // 2M page
        };
        match l1.get_entry(address)? {
            PageTableEntryData::Page4K { contiguous_pages } => Some(PageMeta {
                contiguous_pages: contiguous_pages,
            }),
            _ => unreachable!(), // 2M page
        }
    }

    fn insert_one_page(&mut self, page: Address, num_pages: Option<usize>) {
        let l4 = self;
        let l3 = l4.get_or_allocate_next_page_table(page, || {});
        let l2 = l3.get_or_allocate_next_page_table(page, || { l4.table[L4::get_index(page)].delta_entries(1); });
        let l1 = l2.get_or_allocate_next_page_table(page, || { l3.table[L3::get_index(page)].delta_entries(1); });
        debug_assert!(l1.get_entry(page).is_none());
        l1.table[L1::get_index(page)].set_next_page(num_pages);
    }

    fn insert_pages(&mut self, start: Address, num_pages: usize) {
        for i in 0..num_pages {
            let page = start + (i << 12);
            self.insert_one_page(page, if i == 0 { Some(num_pages) } else { None })
        }
    }

    fn delete_one_page(&mut self, page: Address) {
        let l4 = self;
        let l3 = l4.get_next_page_table(page);
        let l2 = l3.get_next_page_table(page);
        let l1 = l2.get_next_page_table(page);
        debug_assert!(l1.get_entry(page).is_some());
        l1.table[L1::get_index(page)].clear();
        if l2.table[L2::get_index(page)].delta_entries(-1) == 0 {
            l2.table[L2::get_index(page)].clear();
            if l3.table[L3::get_index(page)].delta_entries(-1) == 0 {
                l3.table[L3::get_index(page)].clear();
                if l4.table[L4::get_index(page)].delta_entries(-1) == 0 {
                    l4.table[L4::get_index(page)].clear();
                }
            }
        }
    }

    fn delete_pages(&mut self, start: Address, num_pages: usize) {
        for i in 0..num_pages {
            let page = start + (i << 12);
            self.delete_one_page(page)
        }
    }
}


pub struct PageRegistry {
    p4: RwLock<PageTable<L4>>,
    committed_size: AtomicUsize,
}

impl PageRegistry {
    pub(crate) const fn new() -> Self {
        Self {
            p4: RwLock::new(PageTable::new()),
            committed_size: AtomicUsize::new(0),
        }
    }

    #[inline(always)]
    pub fn committed_size(&self) -> usize {
        self.committed_size.load(Ordering::SeqCst)
    }

    #[inline(always)]
    pub fn is_allocated(&self, address: Address) -> bool {
        self.p4.read().get(address).is_some()
    }

    #[inline(always)]
    pub fn get_contiguous_pages(&self, start: Address) -> usize {
        self.p4.read().get(start).unwrap().contiguous_pages.unwrap()
    }

    pub(crate) fn insert_pages(&self, start: Address, num_pages: usize) {
        self.committed_size.fetch_add(num_pages << 12, Ordering::SeqCst);
        self.p4.write().insert_pages(start, num_pages)
    }

    pub(crate) fn delete_pages(&self, start: Address) -> usize {
        let pages = self.get_contiguous_pages(start);
        self.committed_size.fetch_sub(pages << 12, Ordering::SeqCst);
        self.p4.write().delete_pages(start, pages);
        pages
    }
}