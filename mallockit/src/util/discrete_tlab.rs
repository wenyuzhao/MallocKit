use super::{size_class::SizeClass, Address};

pub struct DiscreteTLAB<const MAX_SIZE_CLASS: usize = { Address::LOG_BYTES }> {
    bins: [Address; MAX_SIZE_CLASS],
    bytes: usize,
}

impl<const MAX_SIZE_CLASS: usize> DiscreteTLAB<MAX_SIZE_CLASS> {
    pub const fn new() -> Self {
        Self {
            bins: [Address::ZERO; MAX_SIZE_CLASS],
            bytes: 0,
        }
    }

    pub fn free_bytes(&self) -> usize {
        self.bytes
    }

    pub fn push(&mut self, size_class: SizeClass, cell: Address) {
        unsafe { cell.store(self.bins[size_class.as_usize()]) };
        self.bins[size_class.as_usize()] = cell;
        self.bytes += size_class.bytes();
    }

    pub fn pop(&mut self, size_class: SizeClass) -> Option<Address> {
        let cell = self.bins[size_class.as_usize()];
        if cell.is_zero() {
            return None;
        }
        self.bins[size_class.as_usize()] = unsafe { cell.load() };
        self.bytes -= size_class.bytes();
        return Some(cell);
    }
}
