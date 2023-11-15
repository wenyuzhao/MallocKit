use crate::space::meta::Meta;
use crate::util::bits::{BitField, BitFieldSlot};
use crate::util::*;
use std::iter::Step;
use std::{marker::PhantomData, mem};

#[repr(transparent)]
struct PageTableEntry<L: PageTableLevel>(usize, PhantomData<L>);

enum PageTableEntryData<L: PageTableLevel> {
    NextLevelPageTable { table: &'static mut PageTable<L> },
    Page { pointer_meta: Address },
}

impl<L: PageTableLevel> PageTableEntry<L> {
    // Global fields
    const PRESENT: BitField = BitField { bits: 1, shift: 63 };
    const IS_PAGE_TABLE: BitField = BitField { bits: 1, shift: 62 };
    // Page table fields
    const PAGE_TABLE_POINTER_MASK: usize = 0x0000_ffff_ffff_f000; // 1: page table, 0: page
    const PAGE_TABLE_USED_ENTRIES: BitField = BitField { bits: 10, shift: 0 };
    // Page fields
    const PAGE_POINTER_META: BitField = BitField { bits: 45, shift: 0 };

    fn clear(&mut self) {
        let value = self.0;
        if value.get(Self::PRESENT) != 0 && value.get(Self::IS_PAGE_TABLE) != 0 {
            let table: &'static mut PageTable<L::NextLevel> =
                unsafe { mem::transmute(value & Self::PAGE_TABLE_POINTER_MASK) };
            let _ = unsafe { Box::from_raw_in(table, Meta) };
        }
        self.0 = 0;
    }

    fn get(&self) -> Option<PageTableEntryData<L::NextLevel>> {
        let value = self.0;
        if value.get(Self::PRESENT) == 0 {
            return None;
        }
        if value.get(Self::IS_PAGE_TABLE) != 0 {
            let table: &'static mut PageTable<L::NextLevel> =
                unsafe { mem::transmute(value & Self::PAGE_TABLE_POINTER_MASK) };
            Some(PageTableEntryData::NextLevelPageTable { table })
        } else {
            let pointer_meta = Address::from(value.get(Self::PAGE_POINTER_META) << 3);
            Some(PageTableEntryData::Page { pointer_meta })
        }
    }

    fn set_next_page_table(&mut self, table: &'static mut PageTable<L::NextLevel>) {
        debug_assert!(self.0 == 0);
        self.0.set(Self::PRESENT, 1);
        self.0.set(Self::IS_PAGE_TABLE, 1);
        self.0.set(Self::PAGE_TABLE_USED_ENTRIES, 0);
        let word = table as *mut _ as usize;
        debug_assert!((word & !Self::PAGE_TABLE_POINTER_MASK) == 0);
        self.0 = self.0 | word;
    }

    fn set_next_page(&mut self, _pages: Option<usize>) {
        debug_assert!(self.0 == 0);
        self.0.set(Self::PRESENT, 1);
        self.0.set(Self::IS_PAGE_TABLE, 0);
    }

    fn delta_entries(&mut self, entries: i32) -> usize {
        self.0.delta(Self::PAGE_TABLE_USED_ENTRIES, entries as _)
    }
}

impl PageTableEntry<L1> {
    fn set_pointer_meta(&mut self, ptr: Address) {
        debug_assert!(self.0.get(Self::PRESENT) != 0);
        debug_assert!(self.0.get(Self::IS_PAGE_TABLE) == 0);
        self.0.set(Self::PAGE_POINTER_META, usize::from(ptr) >> 3);
    }
}

pub(crate) trait PageTableLevel: 'static {
    type NextLevel: PageTableLevel;
    const SHIFT: usize = Self::NextLevel::SHIFT + 9;
    const MASK: usize = 0b1_1111_1111 << Self::SHIFT;

    fn get_index(addr: Address) -> usize {
        (usize::from(addr) & Self::MASK) >> Self::SHIFT
    }
}

pub(crate) struct L4;

impl PageTableLevel for L4 {
    type NextLevel = L3;
}

pub(crate) struct L3;

impl PageTableLevel for L3 {
    type NextLevel = L2;
}

pub(crate) struct L2;

impl PageTableLevel for L2 {
    type NextLevel = L1;
}

pub(crate) struct L1;

impl PageTableLevel for L1 {
    type NextLevel = !;
    const SHIFT: usize = 12;
}

impl PageTableLevel for ! {
    type NextLevel = !;
    const SHIFT: usize = 0;
}

struct PageMeta {
    pub pointer_meta: Address,
}

pub(crate) struct PageTable<L: PageTableLevel = L4> {
    table: [PageTableEntry<L>; 512],
    phantom: PhantomData<L>,
}

impl<L: PageTableLevel> PageTable<L> {
    fn get_entry(&self, address: Address) -> Option<PageTableEntryData<L::NextLevel>> {
        self.table[L::get_index(address)].get()
    }

    fn get_next_page_table(&self, address: Address) -> &'static mut PageTable<L::NextLevel> {
        match self.table[L::get_index(address)].get() {
            Some(PageTableEntryData::NextLevelPageTable { table, .. }) => table,
            _ => unreachable!(),
        }
    }

    fn get_or_allocate_next_page_table(
        &mut self,
        address: Address,
        mut on_create: impl FnMut(),
    ) -> &'static mut PageTable<L::NextLevel> {
        let index = L::get_index(address);
        match self.table[index].get() {
            Some(PageTableEntryData::NextLevelPageTable { table, .. }) => table,
            Some(_) => unreachable!(),
            _ => {
                let table = Box::leak(meta_box!(PageTable::<L::NextLevel> {
                    table: unsafe { mem::transmute([0usize; 512]) },
                    phantom: PhantomData,
                }));
                self.table[index].set_next_page_table(table);
                on_create();
                self.get_next_page_table(address)
            }
        }
    }
}

impl PageTable<L1> {
    fn set_pointer_meta(&mut self, address: Address, pointer_meta: Address) {
        self.table[L1::get_index(address)].set_pointer_meta(pointer_meta);
    }
}

impl PageTable<L4> {
    pub(crate) const fn new() -> Self {
        Self {
            table: unsafe { mem::transmute([0usize; 512]) },
            phantom: PhantomData,
        }
    }

    fn get(&self, address: Address) -> Option<PageMeta> {
        let l3 = match self.get_entry(address)? {
            PageTableEntryData::NextLevelPageTable { table, .. } => table,
            _ => unreachable!(),
        };
        let l2 = match l3.get_entry(address)? {
            PageTableEntryData::NextLevelPageTable { table, .. } => table,
            PageTableEntryData::Page { pointer_meta } => return Some(PageMeta { pointer_meta }),
        };
        let l1 = match l2.get_entry(address)? {
            PageTableEntryData::NextLevelPageTable { table, .. } => table,
            PageTableEntryData::Page { pointer_meta } => return Some(PageMeta { pointer_meta }),
        };
        match l1.get_entry(address)? {
            PageTableEntryData::Page { pointer_meta } => Some(PageMeta { pointer_meta }),
            _ => unreachable!(),
        }
    }

    fn insert_one_page<S: PageSize>(&mut self, start_page: Page<S>, num_pages: Option<usize>) {
        let start = start_page.start();
        let l4 = self;
        let l3 = l4.get_or_allocate_next_page_table(start, || {});
        if S::BYTES == Size1G::BYTES {
            debug_assert!(l3.get_entry(start).is_none());
            l3.table[L3::get_index(start)].set_next_page(num_pages);
            l4.table[L4::get_index(start)].delta_entries(1);
            return;
        }
        let l2 = l3.get_or_allocate_next_page_table(start, || {
            l4.table[L4::get_index(start)].delta_entries(1);
        });
        if S::BYTES == Size2M::BYTES {
            debug_assert!(l2.get_entry(start).is_none());
            l2.table[L2::get_index(start)].set_next_page(num_pages);
            l3.table[L3::get_index(start)].delta_entries(1);
            return;
        }
        let l1 = l2.get_or_allocate_next_page_table(start, || {
            l3.table[L3::get_index(start)].delta_entries(1);
        });
        debug_assert!(l1.get_entry(start).is_none());
        l1.table[L1::get_index(start)].set_next_page(num_pages);
        l2.table[L2::get_index(start)].delta_entries(1);
    }

    pub(crate) fn insert_pages<S: PageSize>(&mut self, start: Page<S>, num_pages: usize) {
        for i in 0..num_pages {
            let page = Step::forward(start, i);
            self.insert_one_page(page, if i == 0 { Some(num_pages) } else { None })
        }
    }

    fn decrease_used_entries<S: PageSize, L: PageTableLevel>(
        parent_table: &mut PageTable<L>,
        page: Page<S>,
    ) -> usize {
        let index = L::get_index(page.start());
        let entries = parent_table.table[index].delta_entries(-1);
        if entries == 0 {
            parent_table.table[index].clear();
        }
        entries
    }

    fn delete_one_page<S: PageSize>(&mut self, start_page: Page<S>) {
        let start = start_page.start();
        let l4 = self;
        let l3 = l4.get_next_page_table(start);
        if S::BYTES == Size1G::BYTES {
            debug_assert!(l3.get_entry(start).is_some());
            l3.table[L3::get_index(start)].clear();
            Self::decrease_used_entries(l4, start_page);
            return;
        }
        let l2 = l3.get_next_page_table(start);
        if S::BYTES == Size2M::BYTES {
            debug_assert!(l2.get_entry(start).is_some());
            l2.table[L2::get_index(start)].clear();
            let dead = Self::decrease_used_entries(l3, start_page) == 0;
            if dead {
                Self::decrease_used_entries(l4, start_page);
            }
            return;
        }
        let l1 = l2.get_next_page_table(start);
        debug_assert!(l1.get_entry(start).is_some());
        l1.table[L1::get_index(start)].clear();
        let dead = Self::decrease_used_entries(l2, start_page) == 0;
        let dead = dead && Self::decrease_used_entries(l3, start_page) == 0;
        if dead {
            Self::decrease_used_entries(l4, start_page);
        }
    }

    pub(crate) fn delete_pages<S: PageSize>(&mut self, start: Page<S>, num_pages: usize) {
        for i in 0..num_pages {
            let page = Step::forward(start, i);
            self.delete_one_page(page)
        }
    }

    pub fn get_pointer_meta(&self, start: Address) -> Address {
        self.get(start).unwrap().pointer_meta
    }

    pub fn set_pointer_meta(&mut self, address: Address, pointer_meta: Address) {
        debug_assert!(usize::from(pointer_meta) & !(((1 << 45) - 1) << 3) == 0);
        let l4 = self;
        let l3 = l4.get_next_page_table(address);
        let l2 = l3.get_next_page_table(address);
        let l1 = l2.get_next_page_table(address);
        l1.set_pointer_meta(address, pointer_meta);
    }
}
