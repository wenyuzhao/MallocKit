use std::ops::Range;

use spin::Lazy;

use crate::space::SpaceId;

use super::{super::sys::raw_memory::RawMemory, address::Address};

const LOG_HEAP_SIZE: usize = 45;
const HEAP_SIZE: usize = 1 << LOG_HEAP_SIZE;

pub static HEAP: Lazy<Heap> = Lazy::new(Heap::new);

pub struct Heap {
    pub(crate) start: Address,
    pub(crate) end: Address,
}

impl Heap {
    fn new() -> Self {
        let start = RawMemory::map_heap(HEAP_SIZE).unwrap();
        let end = start + HEAP_SIZE;
        Self { start, end }
    }

    pub const fn contains(&self, ptr: Address) -> bool {
        self.start <= ptr && ptr < self.end
    }

    pub const fn start(&self) -> Address {
        self.start
    }

    pub const fn end(&self) -> Address {
        self.end
    }

    pub fn get_space_range(&self, id: SpaceId) -> Range<Address> {
        let start = self.start + ((id.0 as usize) << SpaceId::LOG_MAX_SPACE_SIZE);
        let end = start + (1usize << SpaceId::LOG_MAX_SPACE_SIZE);
        start..end
    }
}
