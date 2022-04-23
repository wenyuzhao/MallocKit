use std::{
    cmp::Ordering,
    fmt,
    iter::Step,
    marker::PhantomData,
    num::NonZeroUsize,
    ops::{Deref, DerefMut, Range},
};

use super::Address;

pub trait AlignedBlockConfig {
    const LOG_BYTES: usize;
    type Header: Sized + Send + Sync = ();
}

#[repr(transparent)]
pub struct AlignedBlock<Meta: AlignedBlockConfig>(NonZeroUsize, PhantomData<Meta>);

impl<Meta: AlignedBlockConfig> AlignedBlock<Meta> {
    pub const LOG_BYTES: usize = Meta::LOG_BYTES;
    pub const BYTES: usize = 1 << Meta::LOG_BYTES;
    pub const MASK: usize = Self::BYTES - 1;
    pub const HEADER_BYTES: usize = std::mem::size_of::<Meta::Header>();

    pub const fn new(address: Address) -> Self {
        debug_assert!(!address.is_zero());
        debug_assert!(Self::is_aligned(address));
        Self(
            unsafe { NonZeroUsize::new_unchecked(usize::from(address)) },
            PhantomData,
        )
    }

    pub const fn containing(address: Address) -> Self {
        Self::new(Self::align(address))
    }

    pub const fn align(address: Address) -> Address {
        Address::from(usize::from(address) & !Self::MASK)
    }

    pub const fn is_aligned(address: Address) -> bool {
        (usize::from(address) & Self::MASK) == 0
    }

    pub const fn start(&self) -> Address {
        Address::from(self.0.get())
    }

    pub const fn end(&self) -> Address {
        self.start() + Self::BYTES
    }

    pub const fn range(&self) -> Range<Address> {
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
        return true;
    }
}

impl<Meta: AlignedBlockConfig> fmt::Debug for AlignedBlock<Meta> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "<{:?}>", self.range())
    }
}

unsafe impl<Meta: AlignedBlockConfig> const Send for AlignedBlock<Meta> {}
unsafe impl<Meta: AlignedBlockConfig> const Sync for AlignedBlock<Meta> {}

impl<Meta: AlignedBlockConfig> const Clone for AlignedBlock<Meta> {
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }

    fn clone_from(&mut self, source: &Self) {
        *self = source.clone()
    }
}

impl<Meta: AlignedBlockConfig> const Copy for AlignedBlock<Meta> {}

impl<Meta: AlignedBlockConfig> const PartialEq for AlignedBlock<Meta> {
    fn eq(&self, other: &Self) -> bool {
        self.0.get() == other.0.get()
    }

    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

impl<Meta: AlignedBlockConfig> const Eq for AlignedBlock<Meta> {
    fn assert_receiver_is_total_eq(&self) {}
}

impl<Meta: AlignedBlockConfig> const PartialOrd for AlignedBlock<Meta> {
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

impl<Meta: AlignedBlockConfig> const Ord for AlignedBlock<Meta> {
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
        debug_assert!(min <= max);
        if self < min {
            min
        } else if self > max {
            max
        } else {
            self
        }
    }
}

impl<Meta: AlignedBlockConfig> const Step for AlignedBlock<Meta> {
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

impl<Meta: AlignedBlockConfig> const Deref for AlignedBlock<Meta> {
    type Target = Meta::Header;
    fn deref(&self) -> &Self::Target {
        unsafe { &*self.start().as_ptr() }
    }
}

impl<Meta: AlignedBlockConfig> const DerefMut for AlignedBlock<Meta> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { &mut *self.start().as_mut_ptr() }
    }
}
