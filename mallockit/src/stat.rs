use std::sync::atomic::{AtomicUsize, Ordering};



pub static TOTAL_ALLOCATIONS: Counter = Counter::new();
pub static LARGE_ALLOCATIONS: Counter = Counter::new();
pub static TOTAL_DEALLOCATIONS: Counter = Counter::new();
pub static LARGE_DEALLOCATIONS: Counter = Counter::new();

pub struct Counter(AtomicUsize);

impl Counter {
    pub const fn new() -> Self {
        Self(AtomicUsize::new(0))
    }
    #[inline(always)]
    pub fn get(&self) -> usize {
        self.0.load(Ordering::SeqCst)
    }
    #[inline(always)]
    pub fn inc(&self, delta: usize) {
        self.0.fetch_add(delta, Ordering::SeqCst);
    }
}


pub(crate) fn report() {
    println!("alloc: {} / {}", LARGE_ALLOCATIONS.get(), TOTAL_ALLOCATIONS.get());
    println!("dealloc {} / {}", LARGE_DEALLOCATIONS.get(), TOTAL_DEALLOCATIONS.get());
}