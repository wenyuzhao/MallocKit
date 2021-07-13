use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct BitField {
    pub bits: usize,
    pub shift: usize,
}

pub trait BitFieldSlot: Sized {
    fn get(&self, field: BitField) -> usize;
    fn set(&self, field: BitField, value: usize);
}

impl BitFieldSlot for AtomicUsize {
    #[inline(always)]
    fn get(&self, field: BitField) -> usize {
        let value = self.load(Ordering::Relaxed);
        (value >> field.shift) & ((1usize << field.bits) - 1)
    }

    #[inline(always)]
    fn set(&self, field: BitField, value: usize) {
        let old_value = self.load(Ordering::Relaxed);
        let mask = ((1usize << field.bits) - 1) << field.shift;
        let shifted_value = value << field.shift;
        debug_assert!((shifted_value & !mask) == 0);
        let new_value = (old_value & !mask) | (value << field.shift);
        self.store(new_value, Ordering::Relaxed);
    }
}
