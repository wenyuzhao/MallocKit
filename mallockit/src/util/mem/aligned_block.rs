use super::address::Address;

#[macro_export]
macro_rules! impl_aligned_block {
    ($t: ty) => {
        impl $t {
            pub const LOG_BYTES: usize =
                <Self as $crate::util::mem::aligned_block::AlignedBlockConfig>::LOG_BYTES;
            pub const BYTES: usize = 1 << Self::LOG_BYTES;
            pub const MASK: usize = Self::BYTES - 1;
            pub const HEADER_BYTES: usize = std::mem::size_of::<
                <Self as $crate::util::mem::aligned_block::AlignedBlockConfig>::Header,
            >();
            pub const WORDS: usize = Self::BYTES >> 3;

            pub fn new(address: Address) -> Self {
                <Self as $crate::util::mem::aligned_block::AlignedBlockConfig>::from_address(
                    address,
                )
            }

            pub fn start(&self) -> Address {
                <Self as $crate::util::mem::aligned_block::AlignedBlockConfig>::into_address(*self)
            }

            pub fn end(&self) -> Address {
                self.start() + Self::BYTES
            }

            pub fn align(address: Address) -> Address {
                Address::from(usize::from(address) & !Self::MASK)
            }

            pub fn containing(address: Address) -> Self {
                Self::new(Self::align(address))
            }

            pub fn is_aligned(address: Address) -> bool {
                (usize::from(address) & Self::MASK) == 0
            }

            pub fn range(&self) -> std::ops::Range<Address> {
                std::ops::Range {
                    start: self.start(),
                    end: self.end(),
                }
            }

            pub fn is_zeroed(&self) -> bool {
                let std::ops::Range { start, end } = self.range();
                let mut a = start;
                while a < end {
                    if unsafe { *a.as_ptr::<u8>() != 0 } {
                        return false;
                    }
                    a += 1;
                }
                return true;
            }
        }

        unsafe impl Send for $t {}
        unsafe impl Sync for $t {}

        impl PartialEq for $t {
            fn eq(&self, other: &Self) -> bool {
                self.0.get() == other.0.get()
            }
        }

        impl Eq for $t {
            fn assert_receiver_is_total_eq(&self) {}
        }

        impl PartialOrd for $t {
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }

            fn lt(&self, other: &Self) -> bool {
                matches!(self.partial_cmp(other), Some(std::cmp::Ordering::Less))
            }

            fn le(&self, other: &Self) -> bool {
                matches!(
                    self.partial_cmp(other),
                    Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                )
            }

            fn gt(&self, other: &Self) -> bool {
                matches!(self.partial_cmp(other), Some(std::cmp::Ordering::Greater))
            }

            fn ge(&self, other: &Self) -> bool {
                matches!(
                    self.partial_cmp(other),
                    Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                )
            }
        }

        impl Ord for $t {
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                match (self.0, other.0) {
                    (x, y) if x.get() == y.get() => std::cmp::Ordering::Equal,
                    (x, y) if x.get() < y.get() => std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Greater,
                }
            }

            fn max(self, other: Self) -> Self {
                match Self::cmp(&self, &other) {
                    std::cmp::Ordering::Less | std::cmp::Ordering::Equal => other,
                    std::cmp::Ordering::Greater => self,
                }
            }

            fn min(self, other: Self) -> Self {
                match Self::cmp(&self, &other) {
                    std::cmp::Ordering::Less | std::cmp::Ordering::Equal => self,
                    std::cmp::Ordering::Greater => other,
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

        impl std::iter::Step for $t {
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
                match Self::forward_checked(start, count) {
                    Some(x) => x,
                    _ => unreachable!(),
                }
            }

            unsafe fn forward_unchecked(start: Self, count: usize) -> Self {
                Self::forward(start, count)
            }

            fn backward(start: Self, count: usize) -> Self {
                match Self::backward_checked(start, count) {
                    Some(x) => x,
                    _ => unreachable!(),
                }
            }

            unsafe fn backward_unchecked(start: Self, count: usize) -> Self {
                Self::backward(start, count)
            }
        }

        impl std::ops::Deref for $t {
            type Target = <Self as $crate::util::mem::aligned_block::AlignedBlockConfig>::Header;

            fn deref(&self) -> &Self::Target {
                unsafe { &*self.start().as_ptr() }
            }
        }

        impl std::ops::DerefMut for $t {
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *self.start().as_mut_ptr() }
            }
        }

        impl Clone for $t {
            fn clone(&self) -> Self {
                *self
            }
        }

        impl Copy for $t {}

        impl std::fmt::Debug for $t {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                write!(f, "{}({:?})", std::any::type_name::<Self>(), self.start())
            }
        }
    };
}

#[const_trait]
pub trait AlignedBlockConfig: Sized {
    type Header: Sized = ();

    const LOG_BYTES: usize;

    fn from_address(address: Address) -> Self;
    fn into_address(self) -> Address;
}
