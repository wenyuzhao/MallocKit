use std::cell::Cell;
use std::intrinsics::likely;
use std::marker::PhantomData;
use std::mem::MaybeUninit;
use std::ops::{Deref, DerefMut};
use std::sync::atomic::*;
use std::sync::atomic::{AtomicU8, Ordering};

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
    fn force_slow<T, F: FnOnce() -> T>(lazy: &Lazy<T, Self, F>) {
        Lazy::force_slow_thread_local(lazy)
    }
}

pub struct Shared;
impl ThreadLocality for Shared {
    const THREAD_LOCAL: bool = false;
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

impl<T, TL: ThreadLocality, F: FnOnce() -> T> Lazy<T, TL, F> {
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
        #[allow(cast_ref_to_mut)]
        let me: &mut Self = unsafe { &mut *(self as *const Self as *mut Self) };
        me.value.write(v);
        if !TL::THREAD_LOCAL {
            fence(Ordering::SeqCst);
        }
        self.state.store(INITIALIZED, Ordering::SeqCst);
    }

    #[cold]
    fn force_slow(lazy: &Self) {
        let result =
            lazy.state
                .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |state| match state {
                    UNINITIALIZED => Some(INITIALIZING),
                    _ => None,
                });
        match result {
            Ok(UNINITIALIZED) => {
                lazy.force_initialize();
            }
            Err(INITIALIZING) => loop {
                std::hint::spin_loop();
                if INITIALIZED == lazy.state.load(Ordering::SeqCst) {
                    break;
                }
            },
            Err(INITIALIZED) => Self::force(lazy),
            s => unreachable!("Broken state {:?}", s),
        }
    }

    #[cold]
    fn force_slow_thread_local(lazy: &Self) {
        lazy.state.store(INITIALIZING, Ordering::Relaxed);
        lazy.force_initialize();
    }

    pub fn force(lazy: &Self) {
        if likely(INITIALIZED == lazy.state.load(Ordering::Relaxed)) {
            return;
        }
        TL::force_slow(lazy);
    }

    pub unsafe fn as_initialized(&self) -> &T {
        &*self.value.as_ptr()
    }
}

impl<T, TL: ThreadLocality, F: FnOnce() -> T> Deref for Lazy<T, TL, F> {
    type Target = T;
    fn deref(&self) -> &T {
        Lazy::force(self);
        unsafe { &*self.value.as_ptr() }
    }
}

impl<T, TL: ThreadLocality, F: FnOnce() -> T> DerefMut for Lazy<T, TL, F> {
    fn deref_mut(&mut self) -> &mut T {
        Lazy::force(self);
        unsafe { &mut *self.value.as_mut_ptr() }
    }
}

impl<T, TL: ThreadLocality, F: FnOnce() -> T> AsRef<T> for Lazy<T, TL, F> {
    fn as_ref(&self) -> &T {
        self.deref()
    }
}

impl<T: Default, TL: ThreadLocality> Default for Lazy<T, TL, fn() -> T> {
    fn default() -> Self {
        Lazy::new(T::default)
    }
}

unsafe impl<T, TL: ThreadLocality, F: FnOnce() -> T> Send for Lazy<T, TL, F> {}
unsafe impl<T, TL: ThreadLocality, F: FnOnce() -> T> Sync for Lazy<T, TL, F> {}

pub trait LazyVal<T>: Sized + Deref<Target = T> {}

impl<T, F: FnOnce() -> T> LazyVal<T> for Lazy<T, Local, F> {}

impl<T> LazyVal<T> for &T {}

impl<T> LazyVal<T> for &mut T {}

#[macro_export]
macro_rules! lazy {
    ($value: expr) => {
        $crate::util::Lazy::new(|| $value)
    };
}
