#[macro_export]
#[doc(hidden)]
macro_rules! export_rust_global_alloc_api {
    ($plan: expr, $plan_ty: ty) => {
        pub struct Global;

        impl Global {
            pub fn __fix_layout(mut layout: ::std::alloc::Layout) -> Layout {
                if layout.align() < $crate::util::constants::MIN_ALIGNMENT {
                    layout = layout
                        .align_to($crate::util::constants::MIN_ALIGNMENT)
                        .unwrap();
                }
                layout = unsafe { layout.pad_to_align_unchecked() };
                layout
            }
        }

        unsafe impl ::std::alloc::Allocator for Global {
            fn allocate(
                &self,
                mut layout: ::std::alloc::Layout,
            ) -> ::std::result::Result<::std::ptr::NonNull<[u8]>, ::std::alloc::AllocError> {
                layout = Self::__fix_layout(layout);
                let start = <$plan_ty as $crate::Plan>::Mutator::current()
                    .alloc(layout)
                    .unwrap_or($crate::util::Address::ZERO);
                let slice = unsafe {
                    ::std::slice::from_raw_parts_mut(start.as_mut() as *mut u8, layout.size())
                };
                ::std::result::Result::Ok(::std::ptr::NonNull::from(slice))
            }

            fn allocate_zeroed(
                &self,
                mut layout: ::std::alloc::Layout,
            ) -> ::std::result::Result<::std::ptr::NonNull<[u8]>, ::std::alloc::AllocError> {
                layout = Self::__fix_layout(layout);
                let start = <$plan_ty as $crate::Plan>::Mutator::current()
                    .alloc_zeroed(layout)
                    .unwrap_or($crate::util::Address::ZERO);
                let slice = unsafe {
                    ::std::slice::from_raw_parts_mut(start.as_mut() as *mut u8, layout.size())
                };
                ::std::result::Result::Ok(::std::ptr::NonNull::from(slice))
            }

            unsafe fn deallocate(
                &self,
                ptr: ::std::ptr::NonNull<u8>,
                layout: ::std::alloc::Layout,
            ) {
                <$plan_ty as $crate::Plan>::Mutator::current().dealloc(ptr.as_ptr().into())
            }

            unsafe fn grow(
                &self,
                ptr: ::std::ptr::NonNull<u8>,
                old_layout: ::std::alloc::Layout,
                mut new_layout: ::std::alloc::Layout,
            ) -> ::std::result::Result<::std::ptr::NonNull<[u8]>, ::std::alloc::AllocError> {
                debug_assert!(
                    new_layout.size() >= old_layout.size(),
                    "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
                );

                new_layout = Self::__fix_layout(new_layout);
                let start = <$plan_ty as $crate::Plan>::Mutator::current()
                    .realloc(ptr.as_ptr().into(), new_layout)
                    .unwrap_or($crate::util::Address::ZERO);
                let slice = unsafe {
                    ::std::slice::from_raw_parts_mut(start.as_mut() as *mut u8, new_layout.size())
                };
                ::std::result::Result::Ok(::std::ptr::NonNull::from(slice))
            }

            unsafe fn grow_zeroed(
                &self,
                ptr: ::std::ptr::NonNull<u8>,
                old_layout: ::std::alloc::Layout,
                mut new_layout: ::std::alloc::Layout,
            ) -> ::std::result::Result<::std::ptr::NonNull<[u8]>, ::std::alloc::AllocError> {
                debug_assert!(
                    new_layout.size() >= old_layout.size(),
                    "`new_layout.size()` must be greater than or equal to `old_layout.size()`"
                );

                new_layout = Self::__fix_layout(new_layout);
                let start = <$plan_ty as $crate::Plan>::Mutator::current()
                    .realloc_zeroed(ptr.as_ptr().into(), new_layout)
                    .unwrap_or($crate::util::Address::ZERO);
                let slice = unsafe {
                    ::std::slice::from_raw_parts_mut(start.as_mut() as *mut u8, new_layout.size())
                };
                ::std::result::Result::Ok(::std::ptr::NonNull::from(slice))
            }

            unsafe fn shrink(
                &self,
                ptr: ::std::ptr::NonNull<u8>,
                old_layout: ::std::alloc::Layout,
                mut new_layout: ::std::alloc::Layout,
            ) -> ::std::result::Result<::std::ptr::NonNull<[u8]>, ::std::alloc::AllocError> {
                debug_assert!(
                    new_layout.size() <= old_layout.size(),
                    "`new_layout.size()` must be smaller than or equal to `old_layout.size()`"
                );

                new_layout = Self::__fix_layout(new_layout);
                let start = <$plan_ty as $crate::Plan>::Mutator::current()
                    .realloc(ptr.as_ptr().into(), new_layout)
                    .unwrap_or($crate::util::Address::ZERO);
                let slice = unsafe {
                    ::std::slice::from_raw_parts_mut(start.as_mut() as *mut u8, new_layout.size())
                };
                ::std::result::Result::Ok(::std::ptr::NonNull::from(slice))
            }
        }

        unsafe impl ::std::alloc::GlobalAlloc for Global {
            unsafe fn alloc(&self, mut layout: ::std::alloc::Layout) -> *mut u8 {
                layout = Self::__fix_layout(layout);
                <$plan_ty as $crate::Plan>::Mutator::current()
                    .alloc(layout)
                    .unwrap_or($crate::util::Address::ZERO)
                    .into()
            }

            unsafe fn alloc_zeroed(&self, mut layout: ::std::alloc::Layout) -> *mut u8 {
                layout = Self::__fix_layout(layout);
                <$plan_ty as $crate::Plan>::Mutator::current()
                    .alloc_zeroed(layout)
                    .unwrap_or($crate::util::Address::ZERO)
                    .into()
            }

            unsafe fn dealloc(&self, ptr: *mut u8, _layout: ::std::alloc::Layout) {
                <$plan_ty as $crate::Plan>::Mutator::current().dealloc(ptr.into())
            }

            unsafe fn realloc(
                &self,
                ptr: *mut u8,
                layout: ::std::alloc::Layout,
                new_size: usize,
            ) -> *mut u8 {
                let mut new_layout =
                    unsafe { Layout::from_size_align_unchecked(new_size, layout.align()) };
                new_layout = Self::__fix_layout(new_layout);
                <$plan_ty as $crate::Plan>::Mutator::current()
                    .realloc(ptr.into(), new_layout)
                    .unwrap_or($crate::util::Address::ZERO)
                    .into()
            }
        }
    };
}
