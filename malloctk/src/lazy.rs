use std::sync::atomic::{AtomicU8, Ordering};
use std::ops::{Deref, DerefMut};
use std::mem::MaybeUninit;
use std::cell::Cell;
use std::sync::atomic::*;
use std::marker::PhantomData;

const UNINITIALIZED: u8 = 0;
const INITIALIZING: u8 = 1;
const INITIALIZED: u8 = 2;



pub trait ThreadLocality: Sized {
    const THREAD_LOCAL: bool;
    fn force_slow<T, F: FnOnce() -> T>(lazy: &Lazy<T, Self, F>);
}

pub struct Local;
impl ThreadLocality for Local {
    const THREAD_LOCAL: bool = true;
    #[inline(always)]
    fn force_slow<T, F: FnOnce() -> T>(lazy: &Lazy<T, Self, F>) {
        Lazy::force_slow_thread_local(lazy)
    }
}

pub struct Shared;
impl ThreadLocality for Shared {
    const THREAD_LOCAL: bool = false;
    #[inline(always)]
    fn force_slow<T, F: FnOnce() -> T>(lazy: &Lazy<T, Self, F>) {
        Lazy::force_slow(lazy)
    }
}

pub struct Lazy<T, TL: ThreadLocality = Shared, F: FnOnce() -> T = fn() -> T> {
    state: AtomicU8,
    value: MaybeUninit<T>,
    init: Cell<Option<F>>,
    phantom: PhantomData<TL>,
}

impl <T, TL: ThreadLocality, F: FnOnce() -> T> Lazy<T, TL, F> {
    pub const fn new(f: F) -> Self {
        Self {
            state: AtomicU8::new(UNINITIALIZED),
            value: MaybeUninit::uninit(),
            init: Cell::new(Some(f)),
            phantom: PhantomData,
        }
    }

    fn force_initialize(&self) {
        let f: F = self.init.replace(None).unwrap();
        let v: T = f();
        let me: &mut Self = unsafe { &mut *(self as *const Self as *mut Self) };
        me.value.write(v);
        if !TL::THREAD_LOCAL {
            fence(Ordering::SeqCst);
        }
        self.state.store(INITIALIZED, Ordering::SeqCst);
    }

    #[inline(never)]
    fn force_slow(lazy: &Self) {
        let result = lazy.state.fetch_update(Ordering::SeqCst, Ordering::SeqCst, |state| match state {
            UNINITIALIZED => Some(INITIALIZING),
            _ => None,
        });
        match result {
            Ok(UNINITIALIZED) => {
                lazy.force_initialize();
            }
            Err(INITIALIZING) => {
                loop {
                    spin_loop_hint();
                    if INITIALIZED == lazy.state.load(Ordering::SeqCst) {
                        break;
                    }
                }
            }
            Err(INITIALIZED) => {
                Self::force(lazy)
            },
            s => unreachable!("Broken state {:?}", s),
        }
    }

    #[inline(never)]
    fn force_slow_thread_local(lazy: &Self) {
        lazy.state.store(INITIALIZING, Ordering::Relaxed);
        lazy.force_initialize();
    }

    #[inline(always)]
    pub fn force(lazy: &Self) {
        #[allow(unused_unsafe)]
        if likely!(INITIALIZED == lazy.state.load(Ordering::Relaxed)) {
            return
        }
        TL::force_slow(lazy);
    }

    #[inline(always)]
    pub unsafe fn as_inited(&self) -> &T {
        &*self.value.as_ptr()
    }
}

impl <T, TL: ThreadLocality, F: FnOnce() -> T> Deref for Lazy<T, TL, F> {
    type Target = T;
    #[inline(always)]
    fn deref(&self) -> &T {
        Lazy::force(self);
        unsafe { &*self.value.as_ptr() }
    }
}

impl <T, TL: ThreadLocality, F: FnOnce() -> T> DerefMut for Lazy<T, TL, F> {
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut T {
        Lazy::force(self);
        unsafe { &mut *self.value.as_mut_ptr() }
    }
}

impl <T, TL: ThreadLocality, F: FnOnce() -> T> AsRef<T> for Lazy<T, TL, F> {
    #[inline(always)]
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl <T: Default, TL: ThreadLocality> Default for Lazy<T, TL, fn() -> T> {
    #[inline]
    fn default() -> Self {
        Lazy::new(T::default)
    }
}

unsafe impl <T, TL: ThreadLocality, F: FnOnce() -> T> Send for Lazy<T, TL, F> {}
unsafe impl <T, TL: ThreadLocality, F: FnOnce() -> T> Sync for Lazy<T, TL, F> {}
