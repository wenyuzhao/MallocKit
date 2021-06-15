use std::alloc::Layout;
use std::intrinsics::unlikely;
use std::ptr;

use crate::plan::Plan;
use crate::util::Address;

pub trait Mutator: Sized + 'static + TLS {
    type Plan: Plan<Mutator = Self>;
    const NEW: Self;

    #[inline(always)]
    fn current() -> &'static mut Self {
        <Self as TLS>::current()
    }

    #[inline(always)]
    fn plan() -> &'static Self::Plan {
        Self::Plan::get()
    }

    #[inline(always)]
    fn get_layout(&self, ptr: Address) -> Layout {
        Self::plan().get_layout(ptr)
    }

    fn alloc(&mut self, layout: Layout) -> Option<Address>;

    #[inline(always)]
    fn alloc_zeroed(&mut self, layout: Layout) -> Option<Address> {
        let size = layout.size();
        let ptr = self.alloc(layout);
        if let Some(ptr) = ptr {
            unsafe { ptr::write_bytes(ptr.as_mut_ptr::<u8>(), 0, size) };
        }
        ptr
    }

    fn dealloc(&mut self, ptr: Address);

    #[inline(always)]
    fn realloc(&mut self, ptr: Address, new_size: usize) -> Option<Address> {
        let layout = self.get_layout(ptr);
        if unlikely(layout.size() >= new_size) {
            return Some(ptr);
        }
        let new_layout = unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
        let new_ptr = self.alloc(new_layout);
        if let Some(new_ptr) = new_ptr {
            unsafe {
                ptr::copy_nonoverlapping(
                    ptr.as_ptr::<u8>(),
                    new_ptr.as_mut_ptr::<u8>(),
                    usize::min(layout.size(), new_size),
                );
            }
            self.dealloc(ptr);
        }
        new_ptr
    }
}

pub trait TLS: Sized {
    const NEW: Self;

    #[cfg(not(target_os = "macos"))]
    fn current() -> &'static mut Self;

    #[cfg(target_os = "macos")]
    #[inline(always)]
    fn current() -> &'static mut Self {
        let ptr = macos_tls::get_tls::<Self>();
        if std::intrinsics::unlikely(ptr.is_null()) {
            unsafe { &mut *macos_tls::init_tls::<Self>() }
        } else {
            unsafe { &mut *ptr }
        }
    }
}

#[cfg(target_os = "macos")]
mod macos_tls {
    use super::TLS;
    use crate::space::meta::Meta;

    const SLOT: usize = 89;
    const OFFSET: usize = SLOT * std::mem::size_of::<usize>();

    #[inline(always)]
    #[allow(unused)]
    pub(super) fn get_tls<T: TLS>() -> *mut T {
        unsafe {
            let mut v: *mut T;
            asm!("mov {0}, gs:{offset}", out(reg) v, offset = const OFFSET);
            v
        }
    }

    #[inline(always)]
    #[allow(unused)]
    fn set_tls<T: TLS>(v: *mut T) {
        unsafe {
            asm!("mov gs:{offset}, {0}", in(reg) v, offset = const OFFSET);
        }
    }

    #[cold]
    #[allow(unused)]
    pub(super) fn init_tls<T: TLS>() -> *mut T {
        let ptr = Box::leak(Box::new_in(T::NEW, Meta));
        set_tls::<T>(ptr);
        ptr
    }
}
