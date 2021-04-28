use core::{alloc::Layout,  ptr};
use std::intrinsics::unlikely;
use crate::util::Lazy;
use super::Plan;
use crate::Mutator;


pub struct MallocAPI<GA: Plan>(pub &'static Lazy<GA>);

#[allow(unused)]
impl<P: Plan> MallocAPI<P> {
    #[cfg(not(any(target_os = "macos", all(target_os = "windows", target_pointer_width = "64"))))]
    pub const MIN_ALIGNMENT: usize = 8;
    #[cfg(any(target_os = "macos", all(target_os = "windows", target_pointer_width = "64")))]
    pub const MIN_ALIGNMENT: usize = 16;
    pub const PAGE_SIZE: usize = 4096;

    #[inline(always)]
    pub fn mutator(&self) -> &'static mut P::Mutator {
        P::Mutator::current()
    }

    pub const fn align_up(value: usize, align: usize) -> usize {
        let mask = align - 1;
        (value + mask) & !mask
    }

    #[inline(always)]
    pub fn set_error(e: i32) {
        errno::set_errno(errno::Errno(e));
    }

    #[inline(always)]
    pub unsafe fn alloc(&self, size: usize, align: usize) -> Result<Option<*mut u8>, i32> {
        if cfg!(target_os = "linux") && unlikely(size == 0) { return Ok(None); }
        let size = Self::align_up(size, align);
        let layout = Layout::from_size_align_unchecked(size, align);
        match self.mutator().alloc(layout) {
            Some(ptr) => Ok(Some(ptr.into())),
            None => Err(libc::ENOMEM),
        }
    }

    #[inline(always)]
    pub unsafe fn alloc_or_enomem(&self, size: usize, align: usize) -> *mut u8 {
        match self.alloc(size, Self::MIN_ALIGNMENT) {
            Ok(ptr) => ptr.unwrap_or(0 as _),
            _ => {
                Self::set_error(libc::ENOMEM);
                0 as _
            }
        }
    }

    #[inline(always)]
    pub unsafe fn free(&self, ptr: *mut u8) {
        if unlikely(ptr.is_null()) { return; }
        self.mutator().dealloc(ptr.into());
    }

    #[inline(always)]
    pub unsafe fn reallocate_or_enomem(&self, ptr: *mut u8, new_size: usize, free_if_new_size_is_zero: bool, free_if_fail: bool) -> *mut u8 {
        if ptr.is_null() { return self.alloc_or_enomem(new_size, Self::MIN_ALIGNMENT); }
        if unlikely(free_if_new_size_is_zero && new_size == 0) {
            self.free(ptr);
            return ptr::null_mut();
        }
        let new_size = Self::align_up(new_size, Self::MIN_ALIGNMENT);
        match self.mutator().realloc(ptr.into(), new_size) {
            Some(ptr) => ptr.into(),
            None => {
                if free_if_fail {
                    self.free(ptr);
                }
                Self::set_error(libc::ENOMEM);
                0 as _
            }
        }
    }

    #[inline(always)]
    pub unsafe fn posix_memalign(&self, result: *mut *mut u8, alignment: usize, size: usize) -> i32 {
        if unlikely(alignment <= std::mem::size_of::<usize>() || !alignment.is_power_of_two()) { return libc::EINVAL; }
        match self.alloc(size, alignment) {
            Ok(ptr) => {
                *result = ptr.unwrap_or(0 as _);
                0
            },
            Err(e) => e
        }
    }

    #[inline(always)]
    pub unsafe fn memalign(&self, alignment: usize, size: usize) -> *mut u8 {
        let mut result = ptr::null_mut();
        let errno = self.posix_memalign(&mut result, alignment, size);
        if unlikely(result.is_null()) { Self::set_error(errno) }
        result
    }

    #[inline(always)]
    pub unsafe fn aligned_alloc(&self, size: usize, alignment: usize, einval_if_size_is_not_aligned: bool, einval_if_size_is_zero: bool) -> *mut u8 {
        if unlikely(!alignment.is_power_of_two() || (einval_if_size_is_not_aligned && (size & (alignment - 1)) != 0) || (einval_if_size_is_zero && size == 0)) {
            Self::set_error(libc::EINVAL);
            return ptr::null_mut();
        }
        self.memalign(alignment, size)
    }
}

#[macro_export]
macro_rules! export_malloc_api {
    ($plan: expr) => {
        pub mod __mallockit {
            use super::*;
            use $crate::Plan;
            type Malloc = $crate::malloc::MallocAPI<impl $crate::Plan>;
            static MALLOC_IMPL: Malloc = $crate::malloc::MallocAPI(&$plan);

            #[$crate::ctor]
            unsafe fn ctor() {
                $plan.init();
                $crate::__ctor();
            }

            #[no_mangle]
            pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
                MALLOC_IMPL.alloc_or_enomem(size, Malloc::MIN_ALIGNMENT)
            }

            #[cfg(target_os = "macos")]
            #[no_mangle]
            pub unsafe extern "C" fn malloc_size(ptr: *mut u8) -> usize {
                MALLOC_IMPL.mutator().get_layout(ptr.into()).size()
            }

            #[cfg(target_os = "linux")]
            #[no_mangle]
            pub unsafe extern "C" fn malloc_usable_size(ptr: *mut u8) -> usize {
                MALLOC_IMPL.mutator().get_layout(ptr.into()).size()
            }

            #[no_mangle]
            pub unsafe extern "C" fn free(ptr: *mut u8) {
                MALLOC_IMPL.free(ptr)
            }

            #[cfg(target_os = "linux")]
            #[no_mangle]
            pub unsafe extern "C" fn cfree(ptr: *mut u8) {
                MALLOC_IMPL.free(ptr)
            }

            #[no_mangle]
            pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut u8 {
                let size = count * size;
                let ptr = MALLOC_IMPL.alloc_or_enomem(size, Malloc::MIN_ALIGNMENT);
                std::ptr::write_bytes(ptr, 0, size);
                ptr
            }

            #[cfg(any(target_os = "linux", target_os = "macos"))]
            #[no_mangle]
            pub unsafe extern "C" fn valloc(size: usize) -> *mut u8 {
                MALLOC_IMPL.alloc_or_enomem(size, Malloc::PAGE_SIZE)
            }

            #[cfg(target_os = "linux")]
            #[no_mangle]
            pub unsafe extern "C" fn pvalloc(size: usize) -> *mut u8 {
                MALLOC_IMPL.alloc_or_enomem(size, Malloc::PAGE_SIZE)
            }

            #[no_mangle]
            pub unsafe extern "C" fn realloc(ptr: *mut u8, size: usize) -> *mut u8 {
                MALLOC_IMPL.reallocate_or_enomem(ptr, size, cfg!(any(target_os = "linux", target_os = "windows")), false)
            }

            #[cfg(target_os = "macos")]
            #[no_mangle]
            pub unsafe extern "C" fn reallocf(ptr: *mut u8, size: usize) -> *mut u8 {
                MALLOC_IMPL.reallocate_or_enomem(ptr, size, false, true)
            }

            #[cfg(any(target_os = "linux", target_os = "macos"))]
            #[no_mangle]
            pub unsafe extern "C" fn posix_memalign(ptr: *mut *mut u8, alignment: usize, size: usize) -> i32 {
                MALLOC_IMPL.posix_memalign(ptr, alignment, size)
            }

            #[cfg(target_os = "linux")]
            #[no_mangle]
            pub unsafe extern "C" fn memalign(alignment: usize, size: usize) -> *mut u8 {
                MALLOC_IMPL.memalign(alignment, size)
            }

            #[cfg(target_os = "linux")]
            #[no_mangle]
            pub unsafe extern "C" fn aligned_alloc(alignment: usize, size: usize) -> *mut u8 {
                MALLOC_IMPL.aligned_alloc(size, alignment, true, false)
            }

            #[cfg(target_os = "windows")]
            #[no_mangle]
            pub unsafe extern "C" fn _aligned_malloc(size: usize, alignment: usize) -> *mut u8 {
                MALLOC_IMPL.aligned_alloc(size, alignment, false, true)
            }
        }
    };
}
