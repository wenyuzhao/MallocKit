use super::address_non_null::AddressNonNull;
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

    pub const fn is_zero(&self) -> bool {
        self.0 == 0
    }

    pub const fn is_aligned_to(&self, align: usize) -> bool {
        (self.0 & (align - 1)) == 0
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

    #[inline(always)]
    pub unsafe fn load<T: 'static + Copy>(&self) -> T {
        debug_assert!(!self.is_zero());
        *self.as_ref()
    }

    #[inline(always)]
    pub unsafe fn store<T: 'static + Copy>(&self, value: T) {
        debug_assert!(!self.is_zero());
        *self.as_mut() = value
    }
}

unsafe impl const Send for Address {}
unsafe impl const Sync for Address {}

impl const Clone for Address {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl const Copy for Address {}

impl const From<usize> for Address {
    fn from(value: usize) -> Self {
        Self(value)
    }
}

impl<T> const From<*const T> for Address {
    fn from(value: *const T) -> Self {
        unsafe { Self(value as _) }
    }
}

impl<T> const From<*mut T> for Address {
    fn from(value: *mut T) -> Self {
        unsafe { Self(value as _) }
    }
}

impl<T> const From<&T> for Address {
    fn from(value: &T) -> Self {
        unsafe { Self(value as *const T as _) }
    }
}

impl<T> const From<&mut T> for Address {
    fn from(value: &mut T) -> Self {
        unsafe { Self(value as *const T as _) }
    }
}

impl const From<AddressNonNull> for Address {
    fn from(value: AddressNonNull) -> Self {
        Self(usize::from(value))
    }
}

impl const From<Address> for usize {
    fn from(value: Address) -> usize {
        value.0
    }
}

impl<T> const From<Address> for *const T {
    fn from(value: Address) -> *const T {
        value.0 as _
    }
}

impl<T> const From<Address> for *mut T {
    fn from(value: Address) -> *mut T {
        value.0 as _
    }
}

impl const Deref for Address {
    type Target = usize;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl const PartialEq for Address {
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl const Eq for Address {}

impl const PartialOrd for Address {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl const Ord for Address {
    fn cmp(&self, other: &Self) -> Ordering {
        match (self.0, other.0) {
            (x, y) if x == y => Ordering::Equal,
            (x, y) if x < y => Ordering::Less,
            _ => Ordering::Greater,
        }
    }
}

impl const Add<usize> for Address {
    type Output = Self;
    fn add(self, other: usize) -> Self::Output {
        Self(*self + other)
    }
}

impl const AddAssign<usize> for Address {
    fn add_assign(&mut self, other: usize) {
        *self = *self + other
    }
}

impl const Add<Self> for Address {
    type Output = Self;
    fn add(self, other: Self) -> Self::Output {
        self + *other
    }
}

impl const AddAssign<Self> for Address {
    fn add_assign(&mut self, other: Self) {
        *self = *self + other
    }
}

impl const Add<isize> for Address {
    type Output = Self;
    fn add(self, other: isize) -> Self::Output {
        Self((*self as isize + other) as usize)
    }
}

impl const AddAssign<isize> for Address {
    fn add_assign(&mut self, other: isize) {
        *self = *self + other
    }
}

impl const Add<i32> for Address {
    type Output = Self;
    fn add(self, other: i32) -> Self::Output {
        self + other as isize
    }
}

impl const AddAssign<i32> for Address {
    fn add_assign(&mut self, other: i32) {
        *self = *self + other
    }
}

impl const Sub<Self> for Address {
    type Output = usize;
    fn sub(self, other: Self) -> Self::Output {
        debug_assert!(self.0 >= other.0);
        *self - *other
    }
}

impl const Sub<usize> for Address {
    type Output = Self;
    fn sub(self, other: usize) -> Self::Output {
        Self(self.0 - other)
    }
}

impl const SubAssign<usize> for Address {
    fn sub_assign(&mut self, other: usize) {
        *self = *self - other
    }
}

impl fmt::Debug for Address {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.as_ptr::<u8>())
    }
}

unsafe impl const Step for Address {
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
}
