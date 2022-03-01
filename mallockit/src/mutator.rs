use std::alloc::Layout;
use std::intrinsics::unlikely;
use std::ptr;

use crate::plan::Plan;
use crate::space::meta::MetaLocal;
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
        let layout = Self::Plan::get_layout(ptr);
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

pub(crate) struct InternalTLS {
    pub meta: MetaLocal,
}

impl InternalTLS {
    #[allow(unused)]
    const NEW: Self = Self {
        meta: MetaLocal::new(),
    };

    #[cfg(not(target_os = "macos"))]
    pub fn current() -> &'static mut Self {
        unsafe { &mut INTERNAL_TLS }
    }

    #[cfg(target_os = "macos")]
    pub fn current() -> &'static mut Self {
        let ptr = macos_tls::get_internal_tls();
        unsafe { &mut *ptr }
    }
}

#[cfg(not(target_os = "macos"))]
#[thread_local]
static mut INTERNAL_TLS: InternalTLS = InternalTLS::NEW;

pub trait TLS: Sized {
    const NEW: Self;

    #[cfg(not(target_os = "macos"))]
    fn current() -> &'static mut Self;

    #[cfg(target_os = "macos")]
    #[inline(always)]
    fn current() -> &'static mut Self {
        unsafe { &mut *macos_tls::get_tls::<Self>() }
    }
}

#[cfg(target_os = "macos")]
mod macos_tls {
    use spin::Mutex;
    use std::arch::asm;

    use super::*;
    use crate::util::{memory::RawMemory, AllocationArea, Page, Size4K};

    const SLOT: usize = 89;
    const OFFSET: usize = SLOT * std::mem::size_of::<usize>();

    #[cfg(not(test))]
    extern "C" {
        fn mallockit_initialize_macos_tls() -> *mut u8;
    }

    #[cfg(test)]
    #[no_mangle]
    extern "C" fn mallockit_initialize_macos_tls() -> *mut u8 {
        impl TLS for u8 {
            const NEW: Self = 0;
        }
        get_tls::<u8>()
    }

    #[inline(always)]
    #[allow(unused)]
    fn _get_tls<T>() -> *mut T {
        unsafe {
            let mut v: *mut T;
            asm!("mov {0}, gs:{offset}", out(reg) v, offset = const OFFSET);
            v
        }
    }

    #[inline(always)]
    #[allow(unused)]
    pub(super) fn get_internal_tls() -> *mut InternalTLS {
        let mut tls = _get_tls::<InternalTLS>();
        if unlikely(tls.is_null()) {
            unsafe {
                mallockit_initialize_macos_tls();
            }
            tls = _get_tls::<InternalTLS>();
        }
        debug_assert!(!tls.is_null());
        tls
    }

    #[inline(always)]
    #[allow(unused)]
    pub(super) fn get_tls<T: TLS>() -> *mut T {
        let mut tls = _get_tls::<(InternalTLS, T)>();
        if unlikely(tls.is_null()) {
            tls = init_tls::<T>();
        }
        unsafe { &mut (*tls).1 }
    }

    fn alloc_tls<T>() -> *mut T {
        static ALLOC_BUFFER: Mutex<AllocationArea> = Mutex::new(AllocationArea::EMPTY);

        let layout = Layout::new::<T>();
        if layout.size() > Page::<Size4K>::MASK / 2 {
            RawMemory::map_anonymous(layout.size()).unwrap().into()
        } else {
            let mut buffer = ALLOC_BUFFER.lock();
            if let Some(a) = buffer.alloc(layout) {
                return a.into();
            } else {
                let size = layout.size() << 4;
                let size = (size + Page::<Size4K>::MASK) & !Page::<Size4K>::MASK;
                let top = RawMemory::map_anonymous(size).unwrap();
                let limit = top + size;
                *buffer = AllocationArea { top, limit };
                buffer.alloc(layout).unwrap().into()
            }
        }
    }

    #[cold]
    #[allow(unused)]
    fn init_tls<T: TLS>() -> *mut (InternalTLS, T) {
        let ptr = alloc_tls::<(InternalTLS, T)>();
        unsafe {
            (*ptr).0 = InternalTLS::NEW;
            (*ptr).1 = T::NEW;
            asm!("mov gs:{offset}, {0}", in(reg) ptr, offset = const OFFSET);
        }
        ptr
    }
}
