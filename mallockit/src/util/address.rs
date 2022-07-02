use std::cmp::Ordering;
use std::fmt;
use std::iter::Step;
use std::mem;
use std::ops::{Add, AddAssign, Deref, Sub, SubAssign};

#[repr(transparent)]
pub struct Address(pub(crate) usize);

impl Address {
    pub const LOG_BYTES: usize = mem::size_of::<usize>().trailing_zeros() as usize;
    pub const BYTES: usize = 1 << Self::LOG_BYTES;

    pub const ZERO: Self = Self(0);

    #[inline(always)]
    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }

    #[inline(always)]
    pub const fn align_up(&self, align: usize) -> Address {
        debug_assert!(align.is_power_of_two());
        let mask = align - 1;
        Self((self.0 + mask) & !mask)
    }

    #[inline(always)]
    pub const fn align_down(&self, align: usize) -> Address {
        debug_assert!(align.is_power_of_two());
        let mask = align - 1;
        Self(self.0 & !mask)
    }

    #[inline(always)]
    pub const fn is_aligned_to(&self, align: usize) -> bool {
        debug_assert!(align.is_power_of_two());
        (self.0 & (align - 1)) == 0
    }

    #[inline(always)]
    pub const fn from_usize(v: usize) -> Self {
        Self(v)
    }

    #[inline(always)]
    pub const fn as_usize(&self) -> usize {
        self.0
    }

    #[inline(always)]
    pub const fn as_ptr<T>(&self) -> *const T {
        self.0 as _
    }

    #[inline(always)]
    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.0 as _
    }

    #[inline(always)]
    pub const unsafe fn as_ref<T: 'static>(&self) -> &'static T {
        debug_assert!(!self.is_zero());
        &*self.as_ptr()
    }

    #[inline(always)]
    pub const unsafe fn as_mut<T: 'static>(&self) -> &'static mut T {
        debug_assert!(!self.is_zero());
        &mut *self.as_mut_ptr()
    }

    #[inline(always)]
    pub const unsafe fn load<T: 'static + Copy>(&self) -> T {
        debug_assert!(!self.is_zero());
        *self.as_ref()
    }

    #[inline(always)]
    pub const unsafe fn store<T: 'static + Copy>(&self, value: T) {
        debug_assert!(!self.is_zero());
        *self.as_mut() = value
    }
}

unsafe impl const Send for Address {}
unsafe impl const Sync for Address {}

impl const Clone for Address {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self(self.0)
    }

    #[inline(always)]
    fn clone_from(&mut self, source: &Self) {
        *self = source.clone()
    }
}

impl const Copy for Address {}

impl const From<usize> for Address {
    #[inline(always)]
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl<T> const From<*const T> for Address {
    #[inline(always)]
    fn from(value: *const T) -> Self {
        unsafe { Self(mem::transmute(value)) }
    }
}

impl<T> const From<*mut T> for Address {
    #[inline(always)]
    fn from(value: *mut T) -> Self {
        unsafe { Self(mem::transmute(value)) }
    }
}

impl<T> const From<&T> for Address {
    #[inline(always)]
    fn from(value: &T) -> Self {
        unsafe { Self(mem::transmute(value as *const T)) }
    }
}

impl<T> const From<&mut T> for Address {
    #[inline(always)]
    fn from(value: &mut T) -> Self {
        unsafe { Self(mem::transmute(value as *const T)) }
    }
}

impl const From<Address> for usize {
    #[inline(always)]
    fn from(value: Address) -> usize {
        value.0
    }
}

impl<T> const From<Address> for *const T {
    #[inline(always)]
    fn from(value: Address) -> *const T {
        value.0 as _
    }
}

impl<T> const From<Address> for *mut T {
    #[inline(always)]
    fn from(value: Address) -> *mut T {
        value.0 as _
    }
}

impl const Deref for Address {
    type Target = usize;

    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl const PartialEq for Address {
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }

    #[inline(always)]
    fn ne(&self, other: &Self) -> bool {
        !self.eq(other)
    }
}

impl const Eq for Address {
    fn assert_receiver_is_total_eq(&self) {}
}

impl const PartialOrd for Address {
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }

    #[inline(always)]
    fn lt(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Less))
    }

    #[inline(always)]
    fn le(&self, other: &Self) -> bool {
        matches!(
            self.partial_cmp(other),
            Some(Ordering::Less | Ordering::Equal)
        )
    }

    #[inline(always)]
    fn gt(&self, other: &Self) -> bool {
        matches!(self.partial_cmp(other), Some(Ordering::Greater))
    }

    #[inline(always)]
    fn ge(&self, other: &Self) -> bool {
        matches!(
            self.partial_cmp(other),
            Some(Ordering::Greater | Ordering::Equal)
        )
    }
}

impl const Ord for Address {
    #[inline(always)]
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.0, other.0) {
            (x, y) if x == y => Ordering::Equal,
            (x, y) if x < y => Ordering::Less,
            _ => Ordering::Greater,
        }
    }

    #[inline(always)]
    fn max(self, other: Self) -> Self {
        match Self::cmp(&self, &other) {
            Ordering::Less | Ordering::Equal => other,
            Ordering::Greater => self,
        }
    }

    #[inline(always)]
    fn min(self, other: Self) -> Self {
        match Self::cmp(&self, &other) {
            Ordering::Less | Ordering::Equal => self,
            Ordering::Greater => other,
        }
    }

    #[inline(always)]
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

impl const Add<usize> for Address {
    type Output = Self;

    #[inline(always)]
    fn add(self, other: usize) -> Self::Output {
        Self(*self + other)
    }
}

impl const AddAssign<usize> for Address {
    #[inline(always)]
    fn add_assign(&mut self, other: usize) {
        *self = *self + other
    }
}

impl const Add<Self> for Address {
    type Output = Self;

    #[inline(always)]
    fn add(self, other: Self) -> Self::Output {
        self + *other
    }
}

impl const AddAssign<Self> for Address {
    #[inline(always)]
    fn add_assign(&mut self, other: Self) {
        *self = *self + other
    }
}

impl const Add<isize> for Address {
    type Output = Self;

    #[inline(always)]
    fn add(self, other: isize) -> Self::Output {
        Self((*self as isize + other) as usize)
    }
}

impl const AddAssign<isize> for Address {
    #[inline(always)]
    fn add_assign(&mut self, other: isize) {
        *self = *self + other
    }
}

impl const Add<i32> for Address {
    type Output = Self;

    #[inline(always)]
    fn add(self, other: i32) -> Self::Output {
        self + other as isize
    }
}

impl const AddAssign<i32> for Address {
    #[inline(always)]
    fn add_assign(&mut self, other: i32) {
        *self = *self + other
    }
}

impl const Sub<Self> for Address {
    type Output = usize;

    #[inline(always)]
    fn sub(self, other: Self) -> Self::Output {
        debug_assert!(self.0 >= other.0);
        *self - *other
    }
}

impl const Sub<usize> for Address {
    type Output = Self;

    #[inline(always)]
    fn sub(self, other: usize) -> Self::Output {
        Self(self.0 - other)
    }
}

impl const SubAssign<usize> for Address {
    #[inline(always)]
    fn sub_assign(&mut self, other: usize) {
        *self = *self - other
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_ptr::<u8>())
    }
}

impl const Step for Address {
    #[inline(always)]
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if start.0 > end.0 {
            None
        } else {
            Some(*end - *start)
        }
    }

    #[inline(always)]
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        Some(start + count)
    }

    #[inline(always)]
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        Some(start - count)
    }

    #[inline(always)]
    fn forward(start: Self, count: usize) -> Self {
        Step::forward_checked(start, count).unwrap()
    }

    #[inline(always)]
    unsafe fn forward_unchecked(start: Self, count: usize) -> Self {
        Step::forward(start, count)
    }

    #[inline(always)]
    fn backward(start: Self, count: usize) -> Self {
        Step::backward_checked(start, count).unwrap()
    }

    #[inline(always)]
    unsafe fn backward_unchecked(start: Self, count: usize) -> Self {
        Step::backward(start, count)
    }
}
