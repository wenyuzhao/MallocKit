use std::{intrinsics::likely, marker::PhantomData};

use crate::util::{Page, Size4K};

use super::{memory::RawMemory, Address};

pub struct Arena<T: Sized> {
    freelist: Address,
    phanton: PhantomData<T>,
}

impl<T: Sized> Arena<T> {
    pub const fn new() -> Self {
        Self {
            freelist: Address::ZERO,
            phanton: PhantomData,
        }
    }

    fn push(&mut self, cell: Address) {
        unsafe {
            cell.store(self.freelist);
            self.freelist = cell;
        }
    }

    fn pop(&mut self) -> Option<Address> {
        let cell = self.freelist;
        if likely(!cell.is_zero()) {
            unsafe {
                self.freelist = cell.load();
            };
            Some(cell)
        } else {
            None
        }
    }

    #[cold]
    fn alloc_slow(&mut self) -> Address {
        let obj_size = std::mem::size_of::<T>().next_power_of_two();
        debug_assert!(obj_size <= Page::<Size4K>::BYTES);
        let page = RawMemory::map_anonymous(Page::<Size4K>::BYTES).unwrap();
        for i in (0..Page::<Size4K>::BYTES).step_by(obj_size) {
            self.push(page + i);
        }
        self.pop().unwrap()
    }

    pub fn alloc(&mut self, t: T) -> &'static mut T {
        let cell = match self.pop() {
            Some(cell) => cell,
            _ => self.alloc_slow(),
        };
        let ptr = unsafe { cell.as_mut() };
        unsafe { std::ptr::write(ptr, t) }
        ptr
    }

    pub fn dealloc(&mut self, obj: &'static mut T) {
        let cell = Address::from(obj);
        self.push(cell)
    }
}
