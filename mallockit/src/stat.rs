use std::{
    alloc::Layout,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
};

use atomic::Atomic;
use spin::{Mutex, Once};

use crate::space::meta::{Meta, Vec};

pub static DEFAULT_COUNTER_GROUP: CounterGroup = CounterGroup::new("default");
pub static ALL_GROUPS: Mutex<Vec<&'static CounterGroup>> = Mutex::new(Vec::new_in(Meta));

pub struct CounterGroup {
    name: &'static str,
    counters: Mutex<Vec<Arc<dyn DynCounter, Meta>>>,
    report_fn: Option<fn()>,
    registered: AtomicBool,
}

impl CounterGroup {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            counters: Mutex::new(Vec::new_in(Meta)),
            report_fn: None,
            registered: AtomicBool::new(false),
        }
    }

    pub const fn with_report_fn(mut self, report_fn: fn()) -> Self {
        self.report_fn = Some(report_fn);
        self
    }

    pub const fn new_counter<T: Default + ToString + Copy + 'static>(
        &'static self,
        name: &'static str,
    ) -> Counter<T> {
        Counter::new_grouped(name, self)
    }

    fn add_counter(&'static self, counter: Arc<dyn DynCounter, Meta>) {
        if self
            .registered
            .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
            .is_ok()
        {
            ALL_GROUPS.lock().push(self);
        }
        self.counters.lock().push(counter);
    }

    pub(crate) fn report(&self) {
        if let Some(report_fn) = self.report_fn.as_ref() {
            report_fn();
        } else {
            eprintln!("{}:", self.name);
            while let Some(c) = self.counters.lock().pop() {
                eprintln!("  {}: {}", c.name(), c.format_value());
            }
        }
    }
}

#[allow(unused)]
trait DynCounter: 'static + Sync + Send {
    fn name(&self) -> &'static str;
    fn format_value(&self) -> String;
}

struct CounterImpl<T: Default + ToString + Copy + 'static> {
    name: &'static str,
    value: Atomic<T>,
}

impl<T: Default + ToString + Copy + 'static> CounterImpl<T> {
    pub fn new(name: &'static str) -> Self {
        Self {
            name,
            value: Atomic::new(T::default()),
        }
    }
}

unsafe impl<T: Default + ToString + Copy + 'static> Send for CounterImpl<T> {}
unsafe impl<T: Default + ToString + Copy + 'static> Sync for CounterImpl<T> {}

impl<T: Default + ToString + Copy + 'static> DynCounter for CounterImpl<T> {
    fn name(&self) -> &'static str {
        self.name
    }
    fn format_value(&self) -> String {
        self.value.load(Ordering::SeqCst).to_string()
    }
}

pub struct Counter<T: Default + ToString + Copy + 'static = usize> {
    name: &'static str,
    inner: Once<Arc<CounterImpl<T>, Meta>>,
    group: *const CounterGroup,
}

unsafe impl<T: Default + ToString + Copy + 'static> Send for Counter<T> {}
unsafe impl<T: Default + ToString + Copy + 'static> Sync for Counter<T> {}

impl<T: Default + ToString + Copy + 'static> Counter<T> {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            inner: Once::new(),
            group: &DEFAULT_COUNTER_GROUP,
        }
    }

    pub const fn new_grouped(name: &'static str, group: &'static CounterGroup) -> Self {
        Self {
            name,
            inner: Once::new(),
            group,
        }
    }

    fn inner(&self) -> &Arc<CounterImpl<T>, Meta> {
        self.inner.call_once(|| {
            let c: Arc<CounterImpl<T>, Meta> = Arc::new_in(CounterImpl::new(self.name), Meta);
            unsafe { &*self.group }.add_counter(c.clone());
            c
        })
    }

    pub fn get(&self) -> T {
        assert!(cfg!(feature = "stat"));
        self.inner().value.load(Ordering::SeqCst)
    }

    pub fn set(&self, value: T) {
        assert!(cfg!(feature = "stat"));
        self.inner().value.store(value, Ordering::SeqCst);
    }

    pub fn swap(&self, value: T) -> T {
        assert!(cfg!(feature = "stat"));
        self.inner().value.swap(value, Ordering::SeqCst)
    }

    pub fn fetch_update<F>(&self, f: impl FnMut(T) -> Option<T>) -> Result<T, T> {
        assert!(cfg!(feature = "stat"));
        self.inner()
            .value
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, f)
    }
}

macro_rules! impl_inc_dec {
    ($t: ty) => {
        impl Counter<$t> {
            pub fn inc(&self, delta: $t) {
                assert!(cfg!(feature = "stat"));
                self.inner().value.fetch_add(delta, Ordering::SeqCst);
            }

            pub fn dec(&self, delta: $t) {
                assert!(cfg!(feature = "stat"));
                self.inner().value.fetch_sub(delta, Ordering::SeqCst);
            }
        }
    };
}

impl_inc_dec!(usize);
impl_inc_dec!(u128);
impl_inc_dec!(u64);
impl_inc_dec!(u32);
impl_inc_dec!(u16);
impl_inc_dec!(u8);

impl_inc_dec!(isize);
impl_inc_dec!(i128);
impl_inc_dec!(i64);
impl_inc_dec!(i32);
impl_inc_dec!(i16);
impl_inc_dec!(i8);

// impl_inc_dec!(f32);
// impl_inc_dec!(f64);
// impl_inc_dec!(u32);
// impl_inc_dec!(u64);

pub static ALLOC_COUNTERS: CounterGroup = CounterGroup::new("alloc").with_report_fn(|| {
    eprintln!("alloc:");
    eprintln!(" total-allocations: {}", TOTAL_ALLOCATIONS.get());
    eprintln!(" large-allocations: {}", LARGE_ALLOCATIONS.get());
    eprintln!(" total-deallocations: {}", TOTAL_DEALLOCATIONS.get());
    eprintln!(" large-deallocations: {}", LARGE_DEALLOCATIONS.get());
    eprintln!("alignment:");
    for (i, c) in ALIGNMENTS.iter().enumerate().take(ALIGNMENTS.len() - 1) {
        eprintln!(" - {} = {}", i, c.get());
    }
    eprintln!(" - others = {}", ALIGNMENTS[ALIGNMENTS.len() - 1].get());
    eprintln!("size:");
    for (i, c) in SIZES.iter().enumerate().take(SIZES.len() - 1) {
        eprintln!(" - {} = {}", i, c.get());
    }
    eprintln!(" - others = {}", SIZES[SIZES.len() - 1].get());
});

static TOTAL_ALLOCATIONS: Counter = ALLOC_COUNTERS.new_counter("total-allocations");
static LARGE_ALLOCATIONS: Counter = ALLOC_COUNTERS.new_counter("large-allocations");
static TOTAL_DEALLOCATIONS: Counter = ALLOC_COUNTERS.new_counter("total-deallocations");
static LARGE_DEALLOCATIONS: Counter = ALLOC_COUNTERS.new_counter("large-deallocations");

/// Power of two alignments from 1 to 1024, plus others.
static ALIGNMENTS: [Counter; 12] = [const { ALLOC_COUNTERS.new_counter("align") }; 12];

/// Power of two sizes from 1b to 2M, plus others.
static SIZES: [Counter; 23] = [const { ALLOC_COUNTERS.new_counter("size") }; 23];

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
        let mut i = layout.align().trailing_zeros() as usize;
        if i >= ALIGNMENTS.len() {
            i = ALIGNMENTS.len() - 1;
        }
        ALIGNMENTS[i].inc(1);
        let mut i = layout.size().next_power_of_two().trailing_zeros() as usize;
        if i >= SIZES.len() {
            i = SIZES.len() - 1;
        }
        SIZES[i].inc(1);
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

pub(crate) fn report() {
    if cfg!(not(feature = "stat")) {
        return;
    }
    for group in ALL_GROUPS.lock().iter() {
        group.report();
    }
}
