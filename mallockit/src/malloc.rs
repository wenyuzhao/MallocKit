use super::Plan;
use crate::space::SpaceId;
use crate::util::Address;
use crate::util::Lazy;
use crate::Mutator;
use core::{alloc::Layout, ptr};
use std::intrinsics::unlikely;

pub trait GetMutatorType {
    type Mutator: Mutator;
}

pub struct MallocAPI<P: Plan>(&'static Lazy<P>);

impl<P: Plan> GetMutatorType for MallocAPI<P> {
    type Mutator = P::Mutator;
}

#[allow(unused)]
impl<P: Plan> MallocAPI<P> {
    #[cfg(not(any(
        target_os = "macos",
        all(target_os = "windows", target_pointer_width = "64")
    )))]
    pub const MIN_ALIGNMENT: usize = 16; // should be 8?
    #[cfg(any(
        target_os = "macos",
        all(target_os = "windows", target_pointer_width = "64")
    ))]
    pub const MIN_ALIGNMENT: usize = 16;
    pub const PAGE_SIZE: usize = 4096;

    pub const fn new(plan: &'static Lazy<P>) -> Self {
        Self(plan)
    }

    pub const fn new_mutator() -> P::Mutator {
        P::Mutator::NEW
    }

    pub fn mutator(&self) -> &'static mut P::Mutator {
        P::Mutator::current()
    }

    pub const fn zero_spaceid(a: Address) -> bool {
        SpaceId::from(a).is_invalid()
    }

    pub const fn align_up(value: usize, align: usize) -> usize {
        let mask = align - 1;
        (value + mask) & !mask
    }

    pub fn set_error(e: i32) {
        errno::set_errno(errno::Errno(e));
    }

    pub unsafe fn malloc_size(&self, ptr: Address) -> usize {
        let ptr = Address::from(ptr);
        #[cfg(target_os = "macos")]
        if unlikely(Self::zero_spaceid(ptr.into())) {
            return crate::util::macos_malloc_zone::external_memory_size(ptr);
        }
        P::get_layout(ptr).size()
    }

    pub unsafe fn alloc(&self, size: usize, align: usize) -> Result<Option<*mut u8>, i32> {
        if cfg!(target_os = "linux") && unlikely(size == 0) {
            return Ok(None);
        }
        let size = Self::align_up(size, align);
        let layout = Layout::from_size_align_unchecked(size, align);
        match self.mutator().alloc(layout) {
            Some(ptr) => Ok(Some(ptr.into())),
            None => Err(libc::ENOMEM),
        }
    }

    pub unsafe fn alloc_or_enomem(&self, size: usize, align: usize) -> *mut u8 {
        match self.alloc(size, align) {
            Ok(ptr) => ptr.unwrap_or(0 as _),
            _ => {
                Self::set_error(libc::ENOMEM);
                0 as _
            }
        }
    }

    pub unsafe fn free(&self, ptr: *mut u8) {
        if unlikely(ptr.is_null()) {
            return;
        }
        #[cfg(target_os = "macos")]
        if unlikely(Self::zero_spaceid(ptr.into())) {
            return;
        }
        self.mutator().dealloc(ptr.into());
    }

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
        if unlikely(free_if_new_size_is_zero && new_size == 0) {
            self.free(ptr);
            return ptr::null_mut();
        }
        let new_size = Self::align_up(new_size, Self::MIN_ALIGNMENT);

        #[cfg(target_os = "macos")]
        if unlikely(Self::zero_spaceid(ptr.into())) {
            let ptr = Address::from(ptr);
            let old_size = crate::util::macos_malloc_zone::external_memory_size(ptr);
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

    pub unsafe fn posix_memalign(
        &self,
        result: *mut *mut u8,
        alignment: usize,
        size: usize,
    ) -> i32 {
        if unlikely(alignment < std::mem::size_of::<usize>() || !alignment.is_power_of_two()) {
            return libc::EINVAL;
        }
        match self.alloc(size, usize::max(alignment, Self::MIN_ALIGNMENT)) {
            Ok(ptr) => {
                *result = ptr.unwrap_or(0 as _);
                0
            }
            Err(e) => e,
        }
    }

    pub unsafe fn memalign(&self, alignment: usize, size: usize) -> *mut u8 {
        let mut result = ptr::null_mut();
        let errno = self.posix_memalign(&mut result, alignment, size);
        if unlikely(result.is_null()) {
            Self::set_error(errno)
        }
        result
    }

    pub unsafe fn aligned_alloc(
        &self,
        size: usize,
        alignment: usize,
        einval_if_size_is_not_aligned: bool,
        einval_if_size_is_zero: bool,
    ) -> *mut u8 {
        if unlikely(
            !alignment.is_power_of_two()
                || (einval_if_size_is_not_aligned && (size & (alignment - 1)) != 0)
                || (einval_if_size_is_zero && size == 0),
        ) {
            Self::set_error(libc::EINVAL);
            return ptr::null_mut();
        }
        self.memalign(alignment, size)
    }
}

#[macro_export]
macro_rules! export_malloc_api {
    ($plan: expr, $plan_ty: ty) => {
        pub mod __mallockit {
            use super::*;
            use $crate::Plan;
            type ConcretePlan = $plan_ty;
            type Malloc = $crate::malloc::MallocAPI<ConcretePlan>;
            static MALLOC_IMPL: Malloc = $crate::malloc::MallocAPI::<ConcretePlan>::new(&$plan);

            #[$crate::ctor]
            unsafe fn ctor() {
                $crate::hooks::process_start(&*$plan);
                $crate::libc::atexit($crate::hooks::process_exit);
            }

            #[cfg(target_os = "macos")]
            #[no_mangle]
            pub extern "C" fn mallockit_initialize_macos_tls() -> *mut u8 {
                MALLOC_IMPL.mutator() as *mut _ as _
            }

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
pub use crate::util::macos_malloc_zone::{MallocZone, MALLOCKIT_MALLOC_ZONE as MACOS_MALLOC_ZONE};

#[cfg(target_os = "macos")]
#[cfg(not(feature = "macos_malloc_zone_override"))]
#[macro_export]
macro_rules! export_malloc_api_macos {
    () => {};
}

#[cfg(target_os = "macos")]
#[cfg(feature = "macos_malloc_zone_override")]
#[macro_export]
macro_rules! export_malloc_api_macos {
    () => {
        #[cfg(target_os = "macos")]
        #[$crate::interpose]
        pub unsafe extern "C" fn malloc_default_zone() -> *mut $crate::malloc::MallocZone {
            &mut $crate::malloc::MACOS_MALLOC_ZONE
        }
    };
}
