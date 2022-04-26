use std::{
    alloc::Layout,
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    },
};

use crossbeam::queue::SegQueue;

use crate::util::Lazy;

static COUNTERS: SegQueue<Arc<Counter>> = SegQueue::new();

pub type CounterRef = Lazy<Arc<Counter>>;

pub const fn define_counter<const NAME: &'static str>() -> Lazy<Arc<Counter>> {
    Lazy::new(|| {
        let c = Arc::new(Counter::new(NAME));
        COUNTERS.push(c.clone());
        c
    })
}

static TOTAL_ALLOCATIONS: Lazy<Arc<Counter>> = define_counter::<"total-allocations">();
static LARGE_ALLOCATIONS: Lazy<Arc<Counter>> = define_counter::<"large-allocations">();
static TOTAL_DEALLOCATIONS: Lazy<Arc<Counter>> = define_counter::<"total-deallocations">();
static LARGE_DEALLOCATIONS: Lazy<Arc<Counter>> = define_counter::<"large-deallocations">();

static ALIGNMENTS: [Counter; 11] = [
    Counter::new(""), // 1
    Counter::new(""), // 2
    Counter::new(""), // 4
    Counter::new(""), // 8
    Counter::new(""), // 16
    Counter::new(""), // 32
    Counter::new(""), // 64
    Counter::new(""), // 128
    Counter::new(""), // 256
    Counter::new(""), // 512
    Counter::new(""), // 1024
];
static OTHER_ALIGNMENT: Counter = Counter::new("");

static SIZES: [Counter; 22] = [
    Counter::new(""), // 1B
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""), // 1K
    Counter::new(""),
    Counter::new(""), // 4K
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""),
    Counter::new(""), // 1M
    Counter::new(""), // 2M
];
static OTHER_SIZE: Counter = Counter::new("");

#[inline(always)]
pub fn run(block: impl Fn()) {
    if cfg!(not(feature = "stat")) {
        return;
    }
    block()
}

#[inline(always)]
pub fn track_allocation(layout: Layout, is_large: bool) {
    run(|| {
        let i = layout.align().trailing_zeros() as usize;
        if i < ALIGNMENTS.len() {
            ALIGNMENTS[i].inc(1);
        } else {
            OTHER_ALIGNMENT.inc(1);
        }
        let i = layout.size().next_power_of_two().trailing_zeros() as usize;
        if i < SIZES.len() {
            SIZES[i].inc(1);
        } else {
            OTHER_SIZE.inc(1);
        }
        if is_large {
            LARGE_ALLOCATIONS.inc(1);
        }
        TOTAL_ALLOCATIONS.inc(1);
    })
}

#[inline(always)]
pub fn track_deallocation(is_large: bool) {
    run(|| {
        if is_large {
            LARGE_DEALLOCATIONS.inc(1);
        }
        TOTAL_DEALLOCATIONS.inc(1);
    })
}

pub struct Counter(&'static str, AtomicUsize);

impl Counter {
    pub const fn new(name: &'static str) -> Self {
        Self(name, AtomicUsize::new(0))
    }
    #[inline(always)]
    pub fn get(&self) -> usize {
        assert!(cfg!(feature = "stat"));
        self.1.load(Ordering::SeqCst)
    }
    #[inline(always)]
    pub fn inc(&self, delta: usize) {
        assert!(cfg!(feature = "stat"));
        self.1.fetch_add(delta, Ordering::SeqCst);
    }
}

#[cfg(not(feature = "stat"))]
pub(crate) fn report() {}

#[cfg(feature = "stat")]
pub(crate) fn report() {
    println!("alignment:");
    for i in 0..ALIGNMENTS.len() {
        println!(" - {} = {}", i, ALIGNMENTS[i].get());
    }
    println!(" - others = {}", OTHER_ALIGNMENT.get());
    println!("");
    println!("size:");
    for i in 0..SIZES.len() {
        println!(" - {} = {}", i, SIZES[i].get());
    }
    println!(" - others = {}", OTHER_SIZE.get());
    println!("");
    while let Some(c) = COUNTERS.pop() {
        println!("{}: {}", c.0, c.get());
    }
}
