use crate::util::constants::MIN_ALIGNMENT;
use crate::util::mem::heap::HEAP;
use crate::util::Address;
use crate::util::Lazy;
use crate::Mutator;
use crate::Plan;
use core::{alloc::Layout, ptr};
use std::marker::PhantomData;

pub trait GetMutatorType {
    type Mutator: Mutator;
}

pub struct MallocAPI<P: Plan>(PhantomData<P>);

impl<P: Plan> GetMutatorType for MallocAPI<P> {
    type Mutator = P::Mutator;
}

#[allow(unused)]
impl<P: Plan> MallocAPI<P> {
    pub const MIN_ALIGNMENT: usize = MIN_ALIGNMENT;
    pub const PAGE_SIZE: usize = 4096;

    pub const fn new(plan: &'static Lazy<P>) -> Self {
        Self(PhantomData)
    }

    pub fn mutator(&self) -> &'static mut P::Mutator {
        P::Mutator::current()
    }

    pub fn is_in_mallockit_heap(a: Address) -> bool {
        HEAP.contains(a)
    }

    pub const fn align_up(value: usize, align: usize) -> usize {
        let mask = align - 1;
        (value + mask) & !mask
    }

    pub fn set_error(e: i32) {
        errno::set_errno(errno::Errno(e));
    }

    /// Get malloc size
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` is a valid heap pointer
    pub unsafe fn malloc_size(&self, ptr: Address) -> usize {
        #[cfg(target_os = "macos")]
        if !Self::is_in_mallockit_heap(ptr) {
            return crate::util::malloc::macos_malloc_zone::external_memory_size(ptr);
        }
        P::get_layout(ptr).size()
    }

    /// Allocate memory
    ///
    /// # Safety
    ///
    /// The caller must ensure that `size` and `align` are valid
    pub unsafe fn alloc(&self, mut size: usize, align: usize) -> Result<Option<*mut u8>, i32> {
        size = std::cmp::max(size, Self::MIN_ALIGNMENT);
        let size = Self::align_up(size, align);
        let layout = Layout::from_size_align_unchecked(size, align);
        match self.mutator().alloc(layout) {
            Some(ptr) => Ok(Some(ptr.into())),
            None => Err(libc::ENOMEM),
        }
    }

    /// Allocate memory or set errno to ENOMEM
    ///
    /// # Safety
    ///
    /// The caller must ensure that `size` and `align` are valid
    pub unsafe fn alloc_or_enomem(&self, size: usize, align: usize) -> *mut u8 {
        match self.alloc(size, align) {
            Ok(ptr) => ptr.unwrap_or(0 as _),
            _ => {
                Self::set_error(libc::ENOMEM);
                0 as _
            }
        }
    }

    /// Free memory
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` is a valid heap pointer
    pub unsafe fn free(&self, ptr: *mut u8) {
        if ptr.is_null() {
            return;
        }
        #[cfg(target_os = "macos")]
        if !Self::is_in_mallockit_heap(ptr.into()) {
            return;
        }
        self.mutator().dealloc(ptr.into());
    }

    /// Reallocate memory
    ///
    /// # Safety
    ///
    /// The caller must ensure that `ptr` is a valid heap pointer and `new_size` is valid
    pub unsafe fn reallocate_or_enomem(
        &self,
        ptr: *mut u8,
        new_size: usize,
        free_if_new_size_is_zero: bool,
        free_if_fail: bool,
    ) -> *mut u8 {
        if ptr.is_null() {
            return self.alloc_or_enomem(new_size, Self::MIN_ALIGNMENT);
        }
        if free_if_new_size_is_zero && new_size == 0 {
            self.free(ptr);
            return ptr::null_mut();
        }
        let new_size = Self::align_up(new_size, Self::MIN_ALIGNMENT);

        #[cfg(target_os = "macos")]
        if !Self::is_in_mallockit_heap(ptr.into()) {
            let ptr = Address::from(ptr);
            let old_size = crate::util::malloc::macos_malloc_zone::external_memory_size(ptr);
            let new_layout =
                unsafe { Layout::from_size_align_unchecked(new_size, Self::MIN_ALIGNMENT) };
            let new_ptr = match self.mutator().alloc(new_layout) {
                Some(ptr) => ptr,
                None => {
                    Self::set_error(libc::ENOMEM);
                    return 0 as _;
                }
            };
            unsafe {
                ptr::copy_nonoverlapping(
                    ptr.as_ptr::<u8>(),
                    new_ptr.as_mut_ptr::<u8>(),
                    std::cmp::min(old_size, new_size),
                );
            }
            return new_ptr.into();
        }

        let layout = Layout::from_size_align_unchecked(new_size, Self::MIN_ALIGNMENT);
        match self.mutator().realloc(ptr.into(), layout) {
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

    /// Memalign
    ///
    /// # Safety
    ///
    /// The caller must ensure that `alignment` and `size` are valid
    pub unsafe fn posix_memalign(
        &self,
        result: *mut *mut u8,
        mut alignment: usize,
        size: usize,
    ) -> i32 {
        if !alignment.is_power_of_two() {
            return libc::EINVAL;
        }
        alignment = std::cmp::max(alignment, Self::MIN_ALIGNMENT);
        match self.alloc(size, usize::max(alignment, Self::MIN_ALIGNMENT)) {
            Ok(ptr) => {
                *result = ptr.unwrap_or(0 as _);
                0
            }
            Err(e) => e,
        }
    }

    /// Memalign
    ///
    /// # Safety
    ///
    /// The caller must ensure that `alignment` and `size` are valid
    pub unsafe fn memalign(&self, alignment: usize, size: usize) -> *mut u8 {
        let mut result = ptr::null_mut();
        let errno = self.posix_memalign(&mut result, alignment, size);
        if result.is_null() {
            Self::set_error(errno)
        }
        result
    }

    /// Aligned alloc
    ///
    /// # Safety
    ///
    /// The caller must ensure that `alignment` and `size` are valid
    pub unsafe fn aligned_alloc(
        &self,
        size: usize,
        alignment: usize,
        einval_if_size_is_not_aligned: bool,
        einval_if_size_is_zero: bool,
    ) -> *mut u8 {
        if !alignment.is_power_of_two()
            || (einval_if_size_is_not_aligned && (size & (alignment - 1)) != 0)
            || (einval_if_size_is_zero && size == 0)
        {
            Self::set_error(libc::EINVAL);
            return ptr::null_mut();
        }
        self.memalign(alignment, size)
    }
}

#[macro_export]
#[doc(hidden)]
macro_rules! export_malloc_api {
    ($plan: expr, $plan_ty: ty) => {
        #[cfg(any(feature = "malloc", feature = "mallockit/malloc"))]
        pub mod __mallockit_malloc_api {
            use super::*;
            use $crate::Plan;
            type Malloc = $crate::util::malloc::MallocAPI<$plan_ty>;
            static MALLOC_IMPL: Malloc = $crate::util::malloc::MallocAPI::<$plan_ty>::new(&$plan);

            #[$crate::interpose]
            pub unsafe extern "C" fn malloc(size: usize) -> *mut u8 {
                MALLOC_IMPL.alloc_or_enomem(size, Malloc::MIN_ALIGNMENT)
            }

            #[cfg(target_os = "macos")]
            #[$crate::interpose]
            pub unsafe extern "C" fn malloc_size(ptr: *mut u8) -> usize {
                MALLOC_IMPL.malloc_size(ptr.into())
            }

            // #[cfg(target_os = "macos")]
            // #[$crate::interpose]
            // pub unsafe fn malloc_good_size(ptr: *mut u8) -> usize {
            //     MALLOC_IMPL.malloc_size(ptr.into())
            // }

            #[cfg(target_os = "linux")]
            #[$crate::interpose]
            pub unsafe extern "C" fn malloc_usable_size(ptr: *mut u8) -> usize {
                MALLOC_IMPL.malloc_size(ptr.into())
            }

            #[$crate::interpose]
            pub unsafe extern "C" fn free(ptr: *mut u8) {
                MALLOC_IMPL.free(ptr)
            }

            #[cfg(target_os = "linux")]
            #[$crate::interpose]
            pub unsafe extern "C" fn cfree(ptr: *mut u8) {
                MALLOC_IMPL.free(ptr)
            }

            #[$crate::interpose]
            pub unsafe extern "C" fn calloc(count: usize, size: usize) -> *mut u8 {
                let size = count * size;
                let ptr = MALLOC_IMPL.alloc_or_enomem(size, Malloc::MIN_ALIGNMENT);
                std::ptr::write_bytes(ptr, 0, size);
                ptr
            }

            #[cfg(any(target_os = "linux", target_os = "macos"))]
            #[$crate::interpose]
            pub unsafe extern "C" fn valloc(size: usize) -> *mut u8 {
                MALLOC_IMPL.alloc_or_enomem(size, Malloc::PAGE_SIZE)
            }

            #[cfg(target_os = "linux")]
            #[$crate::interpose]
            pub unsafe extern "C" fn pvalloc(size: usize) -> *mut u8 {
                MALLOC_IMPL.alloc_or_enomem(size, Malloc::PAGE_SIZE)
            }

            #[$crate::interpose]
            pub unsafe extern "C" fn realloc(ptr: *mut u8, size: usize) -> *mut u8 {
                MALLOC_IMPL.reallocate_or_enomem(
                    ptr,
                    size,
                    cfg!(any(target_os = "linux", target_os = "windows")),
                    false,
                )
            }

            #[cfg(target_os = "macos")]
            #[$crate::interpose]
            pub unsafe extern "C" fn reallocf(ptr: *mut u8, size: usize) -> *mut u8 {
                MALLOC_IMPL.reallocate_or_enomem(ptr, size, false, true)
            }

            #[cfg(any(target_os = "linux", target_os = "macos"))]
            #[$crate::interpose]
            pub unsafe extern "C" fn posix_memalign(
                ptr: *mut *mut u8,
                alignment: usize,
                size: usize,
            ) -> i32 {
                MALLOC_IMPL.posix_memalign(ptr, alignment, size)
            }

            #[cfg(target_os = "linux")]
            #[$crate::interpose]
            pub unsafe extern "C" fn memalign(alignment: usize, size: usize) -> *mut u8 {
                MALLOC_IMPL.memalign(alignment, size)
            }

            #[cfg(target_os = "linux")]
            #[$crate::interpose]
            pub unsafe extern "C" fn aligned_alloc(alignment: usize, size: usize) -> *mut u8 {
                MALLOC_IMPL.aligned_alloc(size, alignment, true, false)
            }

            #[cfg(target_os = "windows")]
            #[$crate::interpose]
            pub unsafe extern "C" fn _aligned_malloc(size: usize, alignment: usize) -> *mut u8 {
                MALLOC_IMPL.aligned_alloc(size, alignment, false, true)
            }
        }
    };
}

#[cfg(target_os = "macos")]
pub use crate::util::malloc::macos_malloc_zone::{
    MallocZone, MALLOCKIT_MALLOC_ZONE as MACOS_MALLOC_ZONE,
};

#[cfg(target_os = "macos")]
#[cfg(not(feature = "macos_malloc_zone_override"))]
#[macro_export]
#[doc(hidden)]
macro_rules! export_malloc_api_macos {
    () => {};
}

#[cfg(target_os = "macos")]
#[cfg(feature = "macos_malloc_zone_override")]
#[macro_export]
#[doc(hidden)]
macro_rules! export_malloc_api_macos {
    () => {
        #[cfg(target_os = "macos")]
        #[$crate::interpose]
        pub unsafe extern "C" fn malloc_default_zone() -> *mut $crate::malloc::MallocZone {
            &mut $crate::malloc::MACOS_MALLOC_ZONE
        }
    };
}
