use std::alloc::Layout;

use crate::{mutator::Mutator, util::Address};

pub trait Plan: Singleton + Sized + 'static {
    type Mutator: Mutator<Plan = Self>;

    fn new() -> Self;
    fn init(&self) {}
    fn get_layout(&self, ptr: Address) -> Layout;

    #[inline(always)]
    fn get() -> &'static Self {
        <Self as Singleton>::singleton()
    }
}

pub trait Singleton: Sized + 'static {
    fn singleton() -> &'static Self;
}
