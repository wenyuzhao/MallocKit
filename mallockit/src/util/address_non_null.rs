use std::{ops::{Add, AddAssign, Deref, Sub, SubAssign}, ptr::NonNull};
use std::mem;
use std::ptr;
use std::cmp::Ordering;
use std::fmt;
use super::address::Address;



#[repr(transparent)]
pub struct AddressNonNull(NonNull<u8>);

impl AddressNonNull {
    pub const LOG_BYTES: usize = mem::size_of::<usize>().trailing_zeros() as usize;
    pub const BYTES: usize = 1 << Self::LOG_BYTES;

    pub const DANGLING: Self = AddressNonNull(NonNull::dangling());

    pub const fn as_ptr<T>(&self) -> *const T {
        self.0.as_ptr() as _
    }

    pub const fn as_mut_ptr<T>(&self) -> *mut T {
        self.0.as_ptr() as _
    }

    pub const unsafe fn as_ref<T: 'static>(&self) -> &'static T {
        &*self.as_ptr()
    }

    pub const unsafe fn as_mut<T: 'static>(&self) -> &'static mut T {
        &mut *self.as_mut_ptr()
    }

    #[inline(always)]
    pub unsafe fn load<T: 'static + Copy>(&self) -> T {
        *self.as_ref()
    }

    #[inline(always)]
    pub unsafe fn store<T: 'static + Copy>(&self, value: T) {
        *self.as_mut() = value
    }
}

unsafe impl const Send for AddressNonNull {}
unsafe impl const Sync for AddressNonNull {}

impl const Clone for AddressNonNull {
    fn clone(&self) -> Self {
        Self(self.0)
    }
}

impl const Copy for AddressNonNull {}

impl const From<usize> for AddressNonNull {
    fn from(value: usize) -> Self {
        debug_assert!(value != 0);
        unsafe { Self(ptr::NonNull::new_unchecked(value as _)) }
    }
}

impl<T> const From<*const T> for AddressNonNull {
    fn from(value: *const T) -> Self {
        debug_assert!(!value.is_null());
        unsafe { Self(ptr::NonNull::new_unchecked(value as _)) }
    }
}

impl<T> const From<*mut T> for AddressNonNull {
    fn from(value: *mut T) -> Self {
        debug_assert!(!value.is_null());
        unsafe { Self(ptr::NonNull::new_unchecked(value as _)) }
    }
}

impl<T> const From<&T> for AddressNonNull {
    fn from(value: &T) -> Self {
        unsafe { Self(ptr::NonNull::new_unchecked(value as *const T as _)) }
    }
}

impl<T> const From<&mut T> for AddressNonNull {
    fn from(value: &mut T) -> Self {
        unsafe { Self(ptr::NonNull::new_unchecked(value as *const T as _)) }
    }
}

impl const From<Address> for AddressNonNull {
    fn from(value: Address) -> Self {
        debug_assert!(!value.is_zero());
        unsafe { Self(ptr::NonNull::new_unchecked(usize::from(value) as _)) }
    }
}

impl const From<AddressNonNull> for usize {
    fn from(value: AddressNonNull) -> usize {
        unsafe { value.0.as_ptr() as _ }
    }
}

impl<T> const From<AddressNonNull> for *const T {
    fn from(value: AddressNonNull) -> *const T {
        value.0.as_ptr() as _
    }
}

impl<T> const From<AddressNonNull> for *mut T {
    fn from(value: AddressNonNull) -> *mut T {
        value.0.as_ptr() as _
    }
}

impl const Deref for AddressNonNull {
    type Target = usize;
    fn deref(&self) -> &Self::Target {
        let slot = self as *const Self as *const usize;
        unsafe { &*slot }
    }
}

impl const PartialEq for AddressNonNull {
    fn eq(&self, other: &Self) -> bool {
        unsafe { self.0.as_ptr() as usize == other.0.as_ptr() as usize }
    }
}

impl const Eq for AddressNonNull {}

impl const PartialOrd for AddressNonNull {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl const Ord for AddressNonNull {
    fn cmp(&self, other: &Self) -> Ordering {
        match (usize::from(*self), usize::from(*other)) {
            (x, y) if x == y => Ordering::Equal,
            (x, y) if x < y => Ordering::Less,
            _ => Ordering::Greater,
        }
    }
}


impl const Add<usize> for AddressNonNull {
    type Output = Self;
    fn add(self, other: usize) -> Self::Output {
        Self::from(Address::from(self) + Address::from(other))
    }
}

impl const AddAssign<usize> for AddressNonNull {
    fn add_assign(&mut self, other: usize) { *self = *self + other }
}

impl const Add<Self> for AddressNonNull {
    type Output = Self;
    fn add(self, other: Self) -> Self::Output {
        self + *other
    }
}

impl const AddAssign<Self> for AddressNonNull {
    fn add_assign(&mut self, other: Self) { *self = *self + other }
}

impl const Add<isize> for AddressNonNull {
    type Output = Self;
    fn add(self, other: isize) -> Self::Output {
        debug_assert!((usize::from(self) as isize + other) != 0);
        Self::from(Address::from(self) + other)
    }
}

impl const AddAssign<isize> for AddressNonNull {
    fn add_assign(&mut self, other: isize) { *self = *self + other }
}

impl const Add<i32> for AddressNonNull {
    type Output = Self;
    fn add(self, other: i32) -> Self::Output {
        self + other as isize
    }
}

impl const AddAssign<i32> for AddressNonNull {
    fn add_assign(&mut self, other: i32) { *self = *self + other }
}

impl const Sub<Self> for AddressNonNull {
    type Output = usize;
    fn sub(self, other: Self) -> Self::Output {
        unsafe { debug_assert!(self.0.as_ptr() as usize >= other.0.as_ptr() as usize); }
        *self - *other
    }
}

impl const Sub<usize> for AddressNonNull {
    type Output = Self;
    fn sub(self, other: usize) -> Self::Output {
        debug_assert!(usize::from(self) > other);
        Self::from(Address::from(self) - other)
    }
}

impl const SubAssign<usize> for AddressNonNull {
    fn sub_assign(&mut self, other: usize) { *self = *self - other; }
}

impl fmt::Debug for AddressNonNull {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{:?}", self.0.as_ptr())
    }
}