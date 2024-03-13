use std::{cmp::Ordering, fmt, iter::Step, marker::PhantomData, num::NonZeroUsize, ops::Range};

use super::Address;

pub trait PageSize: 'static + Sized {
    const NAME: &'static str;
    const LOG_BYTES: usize;
    const BYTES: usize = 1 << Self::LOG_BYTES;
}

pub struct Size4K;

impl PageSize for Size4K {
    const NAME: &'static str = "4K";
    const LOG_BYTES: usize = 12;
}

pub struct Size2M;

impl PageSize for Size2M {
    const NAME: &'static str = "2M";
    const LOG_BYTES: usize = 21;
}

pub struct Size1G;

impl PageSize for Size1G {
    const NAME: &'static str = "1G";
    const LOG_BYTES: usize = 30;
}

#[repr(transparent)]
pub struct Page<S: PageSize = Size4K>(NonZeroUsize, PhantomData<S>);

impl<S: PageSize> Page<S> {
    pub const LOG_BYTES: usize = S::LOG_BYTES;
    pub const BYTES: usize = S::BYTES;
    pub const MASK: usize = S::BYTES - 1;

    pub fn new(address: Address) -> Self {
        debug_assert!(!address.is_zero());
        debug_assert!(Self::is_aligned(address));
        Self(
            unsafe { NonZeroUsize::new_unchecked(usize::from(address)) },
            PhantomData,
        )
    }

    pub fn containing(address: Address) -> Self {
        Self::new(Self::align(address))
    }

    pub fn align(address: Address) -> Address {
        Address::from(usize::from(address) & !Self::MASK)
    }

    pub fn is_aligned(address: Address) -> bool {
        (usize::from(address) & Self::MASK) == 0
    }

    pub fn start(&self) -> Address {
        Address::from(self.0.get())
    }

    pub fn end(&self) -> Address {
        self.start() + Self::BYTES
    }

    pub fn range(&self) -> Range<Address> {
        Range {
            start: self.start(),
            end: self.end(),
        }
    }

    pub fn is_zeroed(&self) -> bool {
        for a in self.range() {
            if unsafe { a.load::<u8>() != 0 } {
                return false;
            }
        }
        true
    }
}

impl<S: PageSize> fmt::Debug for Page<S> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Page{}({:?})", S::NAME, self.start())
    }
}

unsafe impl<S: PageSize> Send for Page<S> {}
unsafe impl<S: PageSize> Sync for Page<S> {}

impl<S: PageSize> Clone for Page<S> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<S: PageSize> Copy for Page<S> {}

impl<S: PageSize> PartialEq for Page<S> {
    fn eq(&self, other: &Self) -> bool {
        self.0.get() == other.0.get()
    }
}

impl<S: PageSize> Eq for Page<S> {
    fn assert_receiver_is_total_eq(&self) {}
}

impl<S: PageSize> PartialOrd for Page<S> {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }

    fn lt(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Less))
    }

    fn le(&self, other: &Self) -> bool {
        matches!(
            self.partial_cmp(other),
            Some(Ordering::Less | Ordering::Equal)
        )
    }

    fn gt(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Greater))
    }

    fn ge(&self, other: &Self) -> bool {
        matches!(
            self.partial_cmp(other),
            Some(Ordering::Greater | Ordering::Equal)
        )
    }
}

impl<S: PageSize> Ord for Page<S> {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.0, other.0) {
            (x, y) if x.get() == y.get() => Ordering::Equal,
            (x, y) if x.get() < y.get() => Ordering::Less,
            _ => Ordering::Greater,
        }
    }

    fn max(self, other: Self) -> Self {
        match Self::cmp(&self, &other) {
            Ordering::Less | Ordering::Equal => other,
            Ordering::Greater => self,
        }
    }

    fn min(self, other: Self) -> Self {
        match Self::cmp(&self, &other) {
            Ordering::Less | Ordering::Equal => self,
            Ordering::Greater => other,
        }
    }

    fn clamp(self, min: Self, max: Self) -> Self {
        assert!(min <= max);
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }
}

impl<S: PageSize> Step for Page<S> {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if start.0.get() > end.0.get() {
            None
        } else {
            Some((end.start() - start.start()) >> Self::LOG_BYTES)
        }
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Self::new(start.start() + (count << Self::LOG_BYTES)))
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Self::new(start.start() - (count << Self::LOG_BYTES)))
    }

    fn forward(start: Self, count: usize) -> Self {
        Step::forward_checked(start, count).unwrap()
    }

    unsafe fn forward_unchecked(start: Self, count: usize) -> Self {
        Step::forward(start, count)
    }
    fn backward(start: Self, count: usize) -> Self {
        Step::backward_checked(start, count).unwrap()
    }
    unsafe fn backward_unchecked(start: Self, count: usize) -> Self {
        Step::backward(start, count)
    }
}
