use super::super::SpaceId;
use super::PageResource;
use crate::util::mem::heap::HEAP;
use crate::util::*;
use atomic::Atomic;
use std::iter::Step;
use std::{
    ops::Range,
    sync::atomic::{AtomicUsize, Ordering},
};

pub trait MemRegion: 'static + Sized + Clone + Copy {
    type Meta = ();

    const LOG_BYTES: usize;
    const BYTES: usize = 1 << Self::LOG_BYTES;

    const META_BYTES: usize = std::mem::size_of::<Self::Meta>().next_power_of_two();

    const DATA_BYTES: usize = Self::BYTES - Self::META_BYTES;

    fn start(&self) -> Address;
    fn from_address(addr: Address) -> Self;

    fn data_start(&self) -> Address {
        self.start() + Self::META_BYTES
    }

    fn end(&self) -> Address {
        self.start() + Self::BYTES
    }

    fn data(&self) -> Range<Address> {
        self.data_start()..self.end()
    }

    fn range(&self) -> Range<Address> {
        self.start()..self.end()
    }

    fn meta(&self) -> &Self::Meta {
        unsafe { &*(self.start().as_ptr::<Self::Meta>()) }
    }

    /// # Safety
    /// The caller must ensure that the block is within its corresponding space, and the block is properly aligned.
    #[allow(clippy::mut_from_ref)]
    unsafe fn meta_mut(&self) -> &mut Self::Meta {
        &mut *(self.start().as_mut_ptr::<Self::Meta>())
    }

    fn is_aligned(addr: Address) -> bool {
        addr.is_aligned_to(Self::BYTES)
    }

    fn align(addr: Address) -> Address {
        addr.align_down(Self::BYTES)
    }

    fn containing(addr: Address) -> Self {
        let start = Self::align(addr);
        Self::from_address(start)
    }
}

pub struct BlockPageResource<B: MemRegion> {
    pub id: SpaceId,
    cursor: Atomic<Address>,
    highwater: Address,
    head: Atomic<Option<B>>,
    reserved_bytes: AtomicUsize,
}

impl<B: MemRegion> BlockPageResource<B> {
    pub fn new(id: SpaceId) -> Self {
        debug_assert!(id.0 < 0b0000_1111);
        debug_assert!(B::LOG_BYTES >= Size4K::LOG_BYTES);
        let range = HEAP.get_space_range(id);
        Self {
            id,
            cursor: Atomic::new(range.start),
            highwater: range.end,
            head: Atomic::new(None),
            reserved_bytes: AtomicUsize::new(0),
        }
    }

    #[cold]
    fn acquire_block_slow<S: PageSize>(&self, pages: usize) -> Option<Range<Page<S>>> {
        debug_assert!(B::LOG_BYTES >= S::LOG_BYTES);
        debug_assert_eq!(pages, 1 << (B::LOG_BYTES - S::LOG_BYTES));
        let block = self
            .cursor
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |a| {
                if a >= self.highwater {
                    None
                } else {
                    Some(a + (1usize << B::LOG_BYTES))
                }
            });
        match block {
            Ok(addr) => {
                let start = Page::<S>::new(addr);
                let end = Step::forward(start, pages);
                Some(start..end)
            }
            Err(_) => None,
        }
    }

    const fn set_next(b: B, next: Option<B>) {
        let a = b.start();
        let next = match next {
            Some(b) => b.start(),
            None => Address::ZERO,
        };
        unsafe { a.store(next) }
    }

    const fn get_next(b: B) -> Option<B> {
        let a = b.start();
        let next: Address = unsafe { a.load() };
        if next.is_zero() {
            None
        } else {
            Some(B::from_address(next))
        }
    }

    pub fn acquire_block(&self) -> Option<B> {
        loop {
            let head = self.head.load(Ordering::Relaxed);
            if let Some(block) = head {
                if self
                    .head
                    .compare_exchange(
                        head,
                        Self::get_next(block),
                        Ordering::Relaxed,
                        Ordering::Relaxed,
                    )
                    .is_ok()
                {
                    self.reserved_bytes.fetch_add(B::BYTES, Ordering::Relaxed);
                    return Some(block);
                }
            } else {
                break;
            }
        }
        let range = self.acquire_block_slow::<Size4K>(B::BYTES >> Size4K::LOG_BYTES)?;
        let block = B::from_address(range.start.start());
        Self::set_next(block, None);
        self.reserved_bytes.fetch_add(B::BYTES, Ordering::Relaxed);
        Some(block)
    }

    pub fn release_block(&self, block: B) {
        loop {
            let head = self.head.load(Ordering::Relaxed);
            Self::set_next(block, head);
            if self
                .head
                .compare_exchange(head, Some(block), Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
        self.reserved_bytes
            .fetch_sub(1 << B::LOG_BYTES, Ordering::Relaxed);
    }
}

impl<B: MemRegion> PageResource for BlockPageResource<B> {
    fn reserved_bytes(&self) -> usize {
        self.reserved_bytes.load(Ordering::Relaxed)
    }

    fn acquire_pages<S: PageSize>(&self, _pages: usize) -> Option<Range<Page<S>>> {
        unreachable!("Use `alloc_block` instead")
    }

    fn release_pages<S: PageSize>(&self, _start: Page<S>) {
        unreachable!("Use `release_block` instead")
    }
}
