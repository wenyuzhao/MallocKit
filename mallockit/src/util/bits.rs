use std::sync::atomic::{AtomicUsize, Ordering};

#[derive(PartialEq, Eq, Clone, Copy)]
pub struct BitField {
    pub bits: usize,
    pub shift: usize,
}

pub trait BitFieldSlot: Sized {
    fn get(&self, field: BitField) -> usize;
    fn set(&mut self, field: BitField, value: usize);
    fn delta(&mut self, field: BitField, delta: isize) -> usize {
        let old = self.get(field);
        let new = if delta > 0 {
            old + (delta as usize)
        } else {
            old - ((-delta) as usize)
        };
        self.set(field, new);
        new
    }
}

impl BitFieldSlot for AtomicUsize {
    fn get(&self, field: BitField) -> usize {
        let value = self.load(Ordering::Relaxed);
        (value >> field.shift) & ((1usize << field.bits) - 1)
    }

    fn set(&mut self, field: BitField, value: usize) {
        let old_value = self.load(Ordering::Relaxed);
        let mask = ((1usize << field.bits) - 1) << field.shift;
        let shifted_value = value << field.shift;
        debug_assert!((shifted_value & !mask) == 0);
        let new_value = (old_value & !mask) | (value << field.shift);
        self.store(new_value, Ordering::Relaxed);
    }
}

impl BitFieldSlot for usize {
    fn get(&self, field: BitField) -> usize {
        let value = *self;
        (value >> field.shift) & ((1usize << field.bits) - 1)
    }

    fn set(&mut self, field: BitField, value: usize) {
        let old_value = *self;
        let mask = ((1usize << field.bits) - 1) << field.shift;
        let shifted_value = value << field.shift;
        debug_assert!((shifted_value & !mask) == 0);
        let new_value = (old_value & !mask) | (value << field.shift);
        *self = new_value;
    }

    fn delta(&mut self, field: BitField, delta: isize) -> usize {
        let old = self.get(field);
        let new = if delta > 0 {
            old + (delta as usize)
        } else {
            old - ((-delta) as usize)
        };
        self.set(field, new);
        new
    }
}
