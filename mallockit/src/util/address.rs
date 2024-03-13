use std::cmp::Ordering;
use std::fmt;
use std::hash::Hash;
use std::hash::Hasher;
use std::iter::Step;
use std::mem;
use std::ops::{Add, AddAssign, Deref, Sub, SubAssign};

use atomic::Atomic;

#[repr(transparent)]
pub struct Address(pub(crate) usize);

impl Address {
    pub const LOG_BYTES: usize = mem::size_of::<usize>().trailing_zeros() as usize;
    pub const BYTES: usize = 1 << Self::LOG_BYTES;

    pub const ZERO: Self = Self(0);

    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub const fn align_up(&self, align: usize) -> Address {
        debug_assert!(align.is_power_of_two());
        let mask = align - 1;
        Self((self.0 + mask) & !mask)
    }

    pub const fn align_down(&self, align: usize) -> Address {
        debug_assert!(align.is_power_of_two());
        let mask = align - 1;
        Self(self.0 & !mask)
    }

    pub const fn is_aligned_to(&self, align: usize) -> bool {
        debug_assert!(align.is_power_of_two());
        (self.0 & (align - 1)) == 0
    }

    pub const fn from_usize(v: usize) -> Self {
        Self(v)
    }

    pub const fn as_usize(&self) -> usize {
        self.0
    }

    pub const fn as_ptr<T>(&self) -> *const T {
        self.0 as _
    }

    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as _
    }

    pub const unsafe fn as_ref<T: 'static>(&self) -> &'static T {
        debug_assert!(!self.is_zero());
        &*self.as_ptr()
    }

    pub const unsafe fn as_mut<T: 'static>(&self) -> &'static mut T {
        debug_assert!(!self.is_zero());
        &mut *self.as_mut_ptr()
    }

    pub const unsafe fn load<T: 'static + Copy>(&self) -> T {
        debug_assert!(!self.is_zero());
        *self.as_ref()
    }

    pub const unsafe fn store<T: 'static + Copy>(&self, value: T) {
        debug_assert!(!self.is_zero());
        *self.as_mut() = value
    }

    pub const unsafe fn atomic<T: 'static>(&self) -> &Atomic<T> {
        self.as_ref()
    }
}

unsafe impl Send for Address {}
unsafe impl Sync for Address {}

impl Clone for Address {
    fn clone(&self) -> Self {
        *self
    }
}

impl Copy for Address {}

impl From<usize> for Address {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl<T> From<*const T> for Address {
    fn from(value: *const T) -> Self {
        Self(value as usize)
    }
}

impl<T> From<*mut T> for Address {
    fn from(value: *mut T) -> Self {
        Self(value as usize)
    }
}

impl<T> From<&T> for Address {
    fn from(value: &T) -> Self {
        Self(value as *const T as usize)
    }
}

impl<T> From<&mut T> for Address {
    fn from(value: &mut T) -> Self {
        Self(value as *const T as usize)
    }
}

impl From<Address> for usize {
    fn from(value: Address) -> usize {
        value.0
    }
}

impl<T> From<Address> for *const T {
    fn from(value: Address) -> *const T {
        value.0 as _
    }
}

impl<T> From<Address> for *mut T {
    fn from(value: Address) -> *mut T {
        value.0 as _
    }
}

impl Deref for Address {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl PartialEq for Address {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl Eq for Address {
    fn assert_receiver_is_total_eq(&self) {}
}

impl PartialOrd for Address {
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

impl Ord for Address {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.0, other.0) {
            (x, y) if x == y => Ordering::Equal,
            (x, y) if x < y => Ordering::Less,
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

impl Add<usize> for Address {
    type Output = Self;

    fn add(self, other: usize) -> Self::Output {
        Self(*self + other)
    }
}

impl AddAssign<usize> for Address {
    fn add_assign(&mut self, other: usize) {
        *self = *self + other
    }
}

impl Add<Self> for Address {
    type Output = Self;

    fn add(self, other: Self) -> Self::Output {
        self + *other
    }
}

impl AddAssign<Self> for Address {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other
    }
}

impl Add<isize> for Address {
    type Output = Self;

    fn add(self, other: isize) -> Self::Output {
        Self((*self as isize + other) as usize)
    }
}

impl AddAssign<isize> for Address {
    fn add_assign(&mut self, other: isize) {
        *self = *self + other
    }
}

impl Add<i32> for Address {
    type Output = Self;

    fn add(self, other: i32) -> Self::Output {
        self + other as isize
    }
}

impl AddAssign<i32> for Address {
    fn add_assign(&mut self, other: i32) {
        *self = *self + other
    }
}

impl Sub<Self> for Address {
    type Output = usize;

    fn sub(self, other: Self) -> Self::Output {
        debug_assert!(self.0 >= other.0);
        *self - *other
    }
}

impl Sub<usize> for Address {
    type Output = Self;

    fn sub(self, other: usize) -> Self::Output {
        Self(self.0 - other)
    }
}

impl SubAssign<usize> for Address {
    fn sub_assign(&mut self, other: usize) {
        *self = *self - other
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_ptr::<u8>())
    }
}

impl Step for Address {
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if start.0 > end.0 {
            None
        } else {
            Some(*end - *start)
        }
    }

    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        Some(start + count)
    }

    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        Some(start - count)
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

impl Hash for Address {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
    }
}
