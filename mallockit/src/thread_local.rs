use crate::util::System;

const SLOT: usize = 89;
const OFFSET: usize = SLOT * std::mem::size_of::<usize>();

pub trait TLS: Sized {
    const NEW: Self;

    #[cfg(not(target_os = "macos"))]
    fn current() -> &'static mut Self;

    #[cfg(target_os = "macos")]
    #[inline(always)]
    fn current() -> &'static mut Self {
        let ptr = get_tls::<Self>();
        if std::intrinsics::unlikely(ptr.is_null()) {
            unsafe { &mut *init_tls::<Self>() }
        } else {
            unsafe { &mut *ptr }
        }
    }
}

#[inline(always)]
#[allow(unused)]
fn get_tls<T: TLS>() -> *mut T {
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
fn init_tls<T: TLS>() -> *mut T {
    let ptr = Box::leak(Box::new_in(T::NEW, System));
    set_tls::<T>(ptr);
    ptr
}
