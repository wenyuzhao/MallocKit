use super::{Address, Page};
use crate::util::Size4K;

#[derive(Debug)]
pub struct MemoryMapError;

pub struct RawMemory {
    _private: (),
}

impl RawMemory {
    pub fn map(start: Address, size: usize) -> Result<Address, MemoryMapError> {
        debug_assert!(
            (size & Page::<Size4K>::MASK) == 0,
            "mmap size is not page aligned"
        );
        let ptr = unsafe {
            #[cfg(target_os = "linux")]
            const MAP_FIXED: libc::c_int = libc::MAP_FIXED_NOREPLACE;
            #[cfg(target_os = "macos")]
            const MAP_FIXED: libc::c_int = 0; // `libc::MAP_FIXED` may trigger EXC_GUARD.
            libc::mmap(
                start.as_mut_ptr(),
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | MAP_FIXED,
                -1,
                0,
            )
        };
        if ptr == libc::MAP_FAILED || ptr != start.as_mut_ptr() {
            Err(MemoryMapError)
        } else {
            Ok(ptr.into())
        }
    }

    pub fn map_anonymous(size: usize) -> Result<Address, MemoryMapError> {
        debug_assert!(
            (size & Page::<Size4K>::MASK) == 0,
            "mmap size is not page aligned"
        );
        let ptr = unsafe {
            libc::mmap(
                0 as _,
                size,
                libc::PROT_READ | libc::PROT_WRITE,
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS,
                -1,
                0,
            )
        };
        if ptr == libc::MAP_FAILED {
            Err(MemoryMapError)
        } else {
            Ok(ptr.into())
        }
    }

    pub fn unmap(start: Address, size: usize) {
        debug_assert!(
            (size & Page::<Size4K>::MASK) == 0,
            "mmap size is not page aligned"
        );
        unsafe {
            libc::munmap(start.as_mut_ptr(), size);
        }
    }
}
