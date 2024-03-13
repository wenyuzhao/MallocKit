use super::{Address, Page};
use crate::util::Size4K;

#[derive(Debug)]
pub struct MemoryMapError;

pub struct RawMemory {
    _private: (),
}

impl RawMemory {
    pub(super) fn map_heap(heap_size: usize) -> Result<Address, MemoryMapError> {
        let mmap_start = RawMemory::map_anonymous(heap_size << 1).unwrap();
        let mmap_end = mmap_start + (heap_size << 1);
        let start = mmap_start.align_up(heap_size);
        let end = start + heap_size;
        if start != mmap_start {
            RawMemory::unmap(mmap_start, start - mmap_start);
        }
        if end != mmap_end {
            RawMemory::unmap(end, mmap_end - end);
        }
        #[cfg(target_os = "linux")]
        if cfg!(feature = "transparent_huge_page") {
            unsafe {
                libc::madvise(start.as_mut_ptr(), heap_size, libc::MADV_HUGEPAGE);
            }
        }
        Ok(start)
    }

    #[allow(unused)]
    fn map(start: Address, size: usize) -> Result<Address, MemoryMapError> {
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
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | MAP_FIXED | libc::MAP_NORESERVE,
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
                libc::MAP_PRIVATE | libc::MAP_ANONYMOUS | libc::MAP_NORESERVE,
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

    pub fn madv_free(start: Address, size: usize) {
        debug_assert!(
            (size & Page::<Size4K>::MASK) == 0,
            "mmap size is not page aligned"
        );
        unsafe {
            libc::madvise(start.as_mut_ptr(), size, libc::MADV_FREE);
        }
    }
}
