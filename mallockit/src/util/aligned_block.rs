use super::Address;

#[macro_export]
macro_rules! impl_aligned_block {
    ($t: ty) => {
        impl $t {
            pub const LOG_BYTES: usize =
                <Self as $crate::util::aligned_block::AlignedBlockConfig>::LOG_BYTES;
            pub const BYTES: usize = 1 << Self::LOG_BYTES;
            pub const MASK: usize = Self::BYTES - 1;
            pub const HEADER_BYTES: usize = std::mem::size_of::<
                <Self as $crate::util::aligned_block::AlignedBlockConfig>::Header,
            >();
            pub const WORDS: usize = Self::BYTES >> 3;

            #[inline(always)]
            pub const fn new(address: Address) -> Self {
                <Self as $crate::util::aligned_block::AlignedBlockConfig>::from_address(address)
            }

            #[inline(always)]
            pub const fn start(&self) -> Address {
                <Self as $crate::util::aligned_block::AlignedBlockConfig>::into_address(*self)
            }

            #[inline(always)]
            pub const fn end(&self) -> Address {
                self.start() + Self::BYTES
            }

            #[inline(always)]
            pub const fn align(address: Address) -> Address {
                Address::from(usize::from(address) & !Self::MASK)
            }

            #[inline(always)]
            pub const fn containing(address: Address) -> Self {
                Self::new(Self::align(address))
            }

            #[inline(always)]
            pub const fn is_aligned(address: Address) -> bool {
                (usize::from(address) & Self::MASK) == 0
            }

            #[inline(always)]
            pub const fn range(&self) -> std::ops::Range<Address> {
                std::ops::Range {
                    start: self.start(),
                    end: self.end(),
                }
            }

            #[inline(always)]
            pub const fn is_zeroed(&self) -> bool {
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

        unsafe impl const Send for $t {}
        unsafe impl const Sync for $t {}

        impl const PartialEq for $t {
            #[inline(always)]
            fn eq(&self, other: &Self) -> bool {
                self.0.get() == other.0.get()
            }

            #[inline(always)]
            fn ne(&self, other: &Self) -> bool {
                !self.eq(other)
            }
        }

        impl const Eq for $t {
            fn assert_receiver_is_total_eq(&self) {}
        }

        impl const PartialOrd for $t {
            #[inline(always)]
            fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
                Some(self.cmp(other))
            }

            #[inline(always)]
            fn lt(&self, other: &Self) -> bool {
                matches!(self.partial_cmp(other), Some(std::cmp::Ordering::Less))
            }

            #[inline(always)]
            fn le(&self, other: &Self) -> bool {
                matches!(
                    self.partial_cmp(other),
                    Some(std::cmp::Ordering::Less | std::cmp::Ordering::Equal)
                )
            }

            #[inline(always)]
            fn gt(&self, other: &Self) -> bool {
                matches!(self.partial_cmp(other), Some(std::cmp::Ordering::Greater))
            }

            #[inline(always)]
            fn ge(&self, other: &Self) -> bool {
                matches!(
                    self.partial_cmp(other),
                    Some(std::cmp::Ordering::Greater | std::cmp::Ordering::Equal)
                )
            }
        }

        impl const Ord for $t {
            #[inline(always)]
            fn cmp(&self, other: &Self) -> std::cmp::Ordering {
                match (self.0, other.0) {
                    (x, y) if x.get() == y.get() => std::cmp::Ordering::Equal,
                    (x, y) if x.get() < y.get() => std::cmp::Ordering::Less,
                    _ => std::cmp::Ordering::Greater,
                }
            }

            #[inline(always)]
            fn max(self, other: Self) -> Self {
                match Self::cmp(&self, &other) {
                    std::cmp::Ordering::Less | std::cmp::Ordering::Equal => other,
                    std::cmp::Ordering::Greater => self,
                }
            }

            #[inline(always)]
            fn min(self, other: Self) -> Self {
                match Self::cmp(&self, &other) {
                    std::cmp::Ordering::Less | std::cmp::Ordering::Equal => self,
                    std::cmp::Ordering::Greater => other,
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

        impl const std::iter::Step for $t {
            #[inline(always)]
            fn steps_between(start: &Self, end: &Self) -> Option<usize> {
                if start.0.get() > end.0.get() {
                    None
                } else {
                    Some((end.start() - start.start()) >> Self::LOG_BYTES)
                }
            }

            #[inline(always)]
            fn forward_checked(start: Self, count: usize) -> Option<Self> {
                Some(Self::new(start.start() + (count << Self::LOG_BYTES)))
            }

            #[inline(always)]
            fn backward_checked(start: Self, count: usize) -> Option<Self> {
                Some(Self::new(start.start() - (count << Self::LOG_BYTES)))
            }

            #[inline(always)]
            fn forward(start: Self, count: usize) -> Self {
                match Self::forward_checked(start, count) {
                    Some(x) => x,
                    _ => unreachable!(),
                }
            }

            #[inline(always)]
            unsafe fn forward_unchecked(start: Self, count: usize) -> Self {
                Self::forward(start, count)
            }

            #[inline(always)]
            fn backward(start: Self, count: usize) -> Self {
                match Self::backward_checked(start, count) {
                    Some(x) => x,
                    _ => unreachable!(),
                }
            }

            #[inline(always)]
            unsafe fn backward_unchecked(start: Self, count: usize) -> Self {
                Self::backward(start, count)
            }
        }

        impl const std::ops::Deref for $t {
            type Target = <Self as $crate::util::aligned_block::AlignedBlockConfig>::Header;

            #[inline(always)]
            fn deref(&self) -> &Self::Target {
                unsafe { &*self.start().as_ptr() }
            }
        }

        impl const std::ops::DerefMut for $t {
            #[inline(always)]
            fn deref_mut(&mut self) -> &mut Self::Target {
                unsafe { &mut *self.start().as_mut_ptr() }
            }
        }

        impl const Clone for $t {
            fn clone(&self) -> Self {
                Self::new(self.start())
            }

            fn clone_from(&mut self, source: &Self) {
                *self = source.clone()
            }
        }

        impl const Copy for $t {}
    };
}

pub trait AlignedBlockConfig: Sized {
    type Header: Sized = ();

    const LOG_BYTES: usize;

    fn from_address(address: Address) -> Self;
    fn into_address(self) -> Address;
}
