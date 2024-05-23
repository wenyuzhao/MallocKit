use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn plan(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let name = &input.ident;
    let result = quote! {
        #input

        mod __mallockit_plan {
            type Plan = super::#name;

            pub(super) static PLAN: ::mallockit::util::Lazy<Plan> = ::mallockit::util::Lazy::new(|| <Plan as ::mallockit::Plan>::new());

            #[cfg(any(feature = "malloc", feature = "mallockit/malloc"))]
            #[::mallockit::ctor]
            unsafe fn ctor() {
                ::mallockit::util::sys::hooks::process_start(&*PLAN);
            }

            #[cfg(target_os = "macos")]
            #[no_mangle]
            pub extern "C" fn mallockit_initialize_macos_tls() -> *mut u8 {
                <Plan as ::mallockit::Plan>::Mutator::current() as *mut ::mallockit::Mutator as _
            }

            impl ::mallockit::plan::Singleton for super::#name {
                fn singleton() -> &'static Self {
                    unsafe { &PLAN }
                }
            }

            ::mallockit::export_malloc_api!(PLAN, super::super::#name);
            ::mallockit::export_rust_global_alloc_api!(super::super::#name);
        }

        pub use __mallockit_plan::__mallockit_rust_api::Global;

        #[cfg(test)]
        mod tests {
            include!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../target/generated_tests.rs"
            ));
            ::mallockit::rust_allocator_tests!(crate::Global);
        }
    };
    result.into()
}

#[proc_macro_attribute]
pub fn mutator(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let name = &input.ident;
    let result = quote! {
        #[repr(align(256))]
        #input

        #[cfg(not(target_os = "macos"))]
        mod __mallockit_mutator {
            #[thread_local]
            pub(super) static mut MUTATOR: ::mallockit::util::Lazy<super::#name, ::mallockit::util::Local> = ::mallockit::util::Lazy::new(|| <super::#name as ::mallockit::Mutator>::new());
        }

        impl ::mallockit::mutator::TLS for #name {
            fn new() -> Self {
                <Self as ::mallockit::Mutator>::new()
            }

            #[cfg(not(target_os = "macos"))]
            fn current() -> &'static mut Self {
                unsafe { &mut *__mallockit_mutator::MUTATOR }
            }
        }
    };
    result.into()
}

#[proc_macro_attribute]
pub fn interpose(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let name = &input.sig.ident;
    let interpose_name = syn::Ident::new(&format!("_interpose_{}", name), name.span());
    let result = quote! {
        #[cfg(target_os = "macos")]
        #[cfg(not(test))]
        pub mod #name {
            #[repr(C)]
            pub struct Interpose {
                _new: *const (),
                _old: *const (),
            }

            #[no_mangle]
            #[allow(non_upper_case_globals)]
            #[link_section = "__DATA,__interpose"]
            pub static mut #interpose_name: Interpose = Interpose {
                _new: super::#name as *const (),
                _old: #name as *const (),
            };

            extern {
                pub fn #name();
            }
        }

        #[cfg(target_os = "macos")]
        #[cfg(not(test))]
        #input

        #[cfg(not(target_os = "macos"))]
        #[cfg(not(test))]
        #[no_mangle]
        #input
    };
    result.into()
}

#[proc_macro_attribute]
pub fn aligned_block(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let name = &input.ident;
    let result = quote! {
        #[repr(transparent)]
        #input

        mallockit::impl_aligned_block!(#name);
    };
    result.into()
}
