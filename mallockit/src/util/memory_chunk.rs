use crate::util::Address;
use std::cmp;
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};
use std::{iter::Step, ops::Range};

pub trait MemoryChunkMeta: 'static + Sized {
    const BYTES: usize = std::mem::size_of::<Self>();
}

impl MemoryChunkMeta for () {}

#[repr(C)]
#[derive(Debug, Copy)]
pub struct MemoryChunk<const LOG_BYTES: usize, Meta: MemoryChunkMeta = ()>(
    Address,
    PhantomData<Meta>,
);

impl<const LOG_BYTES: usize, Meta: MemoryChunkMeta> const Clone for MemoryChunk<LOG_BYTES, Meta> {
    #[inline(always)]
    fn clone(&self) -> Self {
        Self(self.0, PhantomData)
    }
}

impl<const LOG_BYTES: usize, Meta: MemoryChunkMeta> const PartialEq
    for MemoryChunk<LOG_BYTES, Meta>
{
    #[inline(always)]
    fn eq(&self, other: &Self) -> bool {
        self.0 == other.0
    }
}

impl<const LOG_BYTES: usize, Meta: MemoryChunkMeta> const PartialOrd
    for MemoryChunk<LOG_BYTES, Meta>
{
    #[inline(always)]
    fn partial_cmp(&self, other: &Self) -> Option<cmp::Ordering> {
        Some(self.0.cmp(&other.0))
    }
}

impl<const LOG_BYTES: usize, Meta: MemoryChunkMeta> MemoryChunk<LOG_BYTES, Meta> {
    pub const LOG_BYTES: usize = LOG_BYTES;
    pub const BYTES: usize = 1 << Self::LOG_BYTES;
    pub const MASK: usize = Self::BYTES - 1;

    pub const DATA_BYTES: usize = Self::BYTES - std::mem::size_of::<Meta>();

    pub const fn align(address: Address) -> Address {
        address.align_down(Self::BYTES)
    }

    #[inline(always)]
    pub fn from(address: Address) -> Self {
        debug_assert!(address.is_aligned_to(Self::BYTES));
        Self(address, PhantomData)
    }

    #[inline(always)]
    pub fn containing(address: Address) -> Self {
        Self(address.align_down(Self::BYTES), PhantomData)
    }

    pub const fn range(&self) -> Range<Address> {
        let start = self.0;
        let end = Address::from_usize(self.0.as_usize() + Self::BYTES);
        start..end
    }

    pub const fn data(&self) -> Range<Address> {
        let start = Address::from_usize(self.0.as_usize() + std::mem::size_of::<Meta>());
        let end = Address::from_usize(self.0.as_usize() + Self::BYTES);
        start..end
    }
}

impl<const LOG_BYTES: usize, Meta: MemoryChunkMeta> Step for MemoryChunk<LOG_BYTES, Meta> {
    #[inline(always)]
    fn steps_between(start: &Self, end: &Self) -> Option<usize> {
        if start < end {
            return None;
        }
        Some((end.range().start - start.range().start) >> Self::LOG_BYTES)
    }
    #[inline(always)]
    fn forward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Self::from(start.range().start + (count << Self::LOG_BYTES)))
    }
    #[inline(always)]
    fn backward_checked(start: Self, count: usize) -> Option<Self> {
        Some(Self::from(start.range().start - (count << Self::LOG_BYTES)))
    }
}

impl<const LOG_BYTES: usize, Meta: MemoryChunkMeta> const Deref for MemoryChunk<LOG_BYTES, Meta> {
    type Target = Meta;
    #[inline(always)]
    fn deref(&self) -> &Self::Target {
        unsafe { self.0.as_ref() }
    }
}

impl<const LOG_BYTES: usize, Meta: MemoryChunkMeta> const DerefMut
    for MemoryChunk<LOG_BYTES, Meta>
{
    #[inline(always)]
    fn deref_mut(&mut self) -> &mut Self::Target {
        unsafe { self.0.as_mut() }
    }
}
