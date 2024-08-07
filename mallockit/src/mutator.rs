use std::alloc::Layout;
use std::ptr;
#[cfg(not(target_os = "macos"))]
use std::ptr::addr_of_mut;

use crate::plan::Plan;
use crate::space::meta::MetaLocal;
use crate::util::Address;

pub trait Mutator: Sized + 'static + TLS {
    type Plan: Plan<Mutator = Self>;

    fn new() -> Self;

    fn current() -> &'static mut Self {
        <Self as TLS>::current()
    }

    fn plan() -> &'static Self::Plan {
        Self::Plan::get()
    }

    fn alloc(&mut self, layout: Layout) -> Option<Address>;

    fn alloc_zeroed(&mut self, layout: Layout) -> Option<Address> {
        let size = layout.size();
        let ptr = self.alloc(layout);
        if let Some(ptr) = ptr {
            unsafe { ptr::write_bytes(ptr.as_mut_ptr::<u8>(), 0, size) };
        }
        ptr
    }

    fn dealloc(&mut self, ptr: Address);

    fn realloc(&mut self, ptr: Address, new_layout: Layout) -> Option<Address> {
        let layout = Self::Plan::get_layout(ptr);
        if layout.size() >= new_layout.size() && layout.align() >= new_layout.align() {
            return Some(ptr);
        }
        let new_ptr = self.alloc(new_layout);
        if let Some(new_ptr) = new_ptr {
            unsafe {
                ptr::copy_nonoverlapping(
                    ptr.as_ptr::<u8>(),
                    new_ptr.as_mut_ptr::<u8>(),
                    usize::min(layout.size(), new_layout.size()),
                );
            }
            self.dealloc(ptr);
        }
        new_ptr
    }

    fn realloc_zeroed(&mut self, ptr: Address, new_layout: Layout) -> Option<Address> {
        let size = new_layout.size();
        let new_ptr = self.realloc(ptr, new_layout);
        if let Some(new_ptr) = new_ptr {
            unsafe { ptr::write_bytes(new_ptr.as_mut_ptr::<u8>(), 0, size) };
        }
        new_ptr
    }
}

pub(crate) struct InternalTLS {
    pub meta: MetaLocal,
}

impl InternalTLS {
    #[allow(unused)]
    const fn new() -> Self {
        Self {
            meta: MetaLocal::new(),
        }
    }

    #[cfg(not(target_os = "macos"))]
    pub fn current() -> &'static mut Self {
        unsafe { &mut *addr_of_mut!(INTERNAL_TLS) }
    }

    #[cfg(target_os = "macos")]
    pub fn current() -> &'static mut Self {
        let ptr = macos_tls::get_internal_tls();
        unsafe { &mut *ptr }
    }
}

#[cfg(not(target_os = "macos"))]
#[thread_local]
static mut INTERNAL_TLS: InternalTLS = InternalTLS::new();

pub trait TLS: Sized {
    fn new() -> Self;

    #[cfg(not(target_os = "macos"))]
    fn current() -> &'static mut Self;

    #[cfg(target_os = "macos")]
    fn current() -> &'static mut Self {
        unsafe { &mut *macos_tls::get_tls::<Self>() }
    }

    fn reset(&mut self) {
        *self = Self::new();
    }
}

impl TLS for u8 {
    fn new() -> Self {
        0
    }

    #[cfg(not(target_os = "macos"))]
    fn current() -> &'static mut Self {
        unreachable!()
    }
}

#[cfg(target_os = "macos")]
mod macos_tls {
    use spin::{mutex::Mutex, Yield};
    use std::arch::asm;

    use super::*;
    use crate::util::{mem::alloc::allocation_area::AllocationArea, sys::RawMemory, Page, Size4K};

    const SLOT: usize = 89;
    #[cfg(target_arch = "x86_64")]
    const OFFSET: usize = SLOT * std::mem::size_of::<usize>();

    #[cfg(not(test))]
    extern "C" {
        fn mallockit_initialize_macos_tls() -> *mut u8;
    }

    #[cfg(test)]
    #[no_mangle]
    extern "C" fn mallockit_initialize_macos_tls() -> *mut u8 {
        get_tls::<u8>()
    }

    #[allow(unused)]
    #[cfg(target_arch = "x86_64")]
    fn _get_tls<T>() -> *mut T {
        unsafe {
            let mut v: *mut T;
            asm!("mov {0}, gs:{offset}", out(reg) v, offset = const OFFSET);
            v
        }
    }

    #[allow(unused)]
    #[cfg(target_arch = "aarch64")]
    fn _get_tls<T>() -> *mut T {
        unsafe {
            let mut tcb: *mut *mut T;
            asm! {
                "
                mrs {0}, tpidrro_el0
                bic {0}, {0}, #7
                ",
                out(reg) tcb
            }
            tcb.add(SLOT).read()
        }
    }

    #[allow(unused)]
    pub(super) fn get_internal_tls() -> *mut InternalTLS {
        let mut tls = _get_tls::<InternalTLS>();
        if tls.is_null() {
            unsafe {
                mallockit_initialize_macos_tls();
            }
            tls = _get_tls::<InternalTLS>();
        }
        debug_assert!(!tls.is_null());
        tls
    }

    #[allow(unused)]
    pub(super) fn get_tls<T: TLS>() -> *mut T {
        let mut tls = _get_tls::<(InternalTLS, T)>();
        if tls.is_null() {
            tls = init_tls::<T>();
        }
        unsafe { &mut (*tls).1 }
    }

    fn alloc_tls<T>() -> *mut T {
        static ALLOC_BUFFER: Mutex<AllocationArea, Yield> = Mutex::new(AllocationArea::EMPTY);

        let layout = Layout::new::<T>();
        if layout.size() > Page::<Size4K>::MASK / 2 {
            RawMemory::map_anonymous(layout.size()).unwrap().into()
        } else {
            let mut buffer = ALLOC_BUFFER.lock();
            if let Some(a) = buffer.alloc(layout) {
                a.into()
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
    #[cfg(target_arch = "aarch64")]
    fn init_tls<T: TLS>() -> *mut (InternalTLS, T) {
        let ptr = alloc_tls::<(InternalTLS, T)>();
        unsafe {
            std::ptr::write(&mut (*ptr).0, InternalTLS::new());
            unsafe {
                let mut tcb: *mut *mut T;
                asm! {
                    "
                    mrs {0}, tpidrro_el0
                    bic {0}, {0}, #7
                    ",
                    out(reg) tcb
                }
                tcb.add(SLOT).write(ptr as *mut T)
            }
            std::ptr::write(&mut (*ptr).1, T::new());
            crate::mutator::init_pthread_specific();
        }
        ptr
    }

    #[cold]
    #[allow(unused)]
    #[cfg(target_arch = "x86_64")]
    fn init_tls<T: TLS>() -> *mut (InternalTLS, T) {
        let ptr = alloc_tls::<(InternalTLS, T)>();
        unsafe {
            std::ptr::write(&mut (*ptr).0, InternalTLS::new());
            asm!("mov gs:{offset}, {0}", in(reg) ptr, offset = const OFFSET);
            std::ptr::write(&mut (*ptr).1, T::new());
            crate::mutator::init_pthread_specific();
        }
        ptr
    }
}

static mut TLS_KEY: libc::pthread_key_t = libc::pthread_key_t::MAX;

#[thread_local]
static X: usize = 0;

extern "C" {
    fn mallockit_pthread_destructor();
}

extern "C" fn dtor(_ptr: *mut libc::c_void) {
    unsafe {
        mallockit_pthread_destructor();
    }
}

pub fn init_pthread_specific() {
    unsafe {
        libc::pthread_setspecific(TLS_KEY, &X as *const usize as _);
    }
}

pub(crate) fn init_pthread_key() {
    unsafe {
        libc::pthread_key_create(std::ptr::addr_of_mut!(TLS_KEY), Some(dtor));
    }
}
