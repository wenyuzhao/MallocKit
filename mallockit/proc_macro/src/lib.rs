use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn plan(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let name = &input.ident;
    let result = quote! {
        #input

        mod __mallockit_plan {
            pub(super) static PLAN: mallockit::util::Lazy<super::#name> = mallockit::util::Lazy::new(|| <super::#name as mallockit::Plan>::new());

            #[cfg(not(test))]
            mallockit::export_malloc_api!(PLAN);
        }

        impl mallockit::plan::Singleton for #name {
            #[inline(always)]
            fn singleton() -> &'static Self {
                unsafe { &__mallockit_plan::PLAN }
            }
        }

        include!(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../target/generated_tests.rs"
        ));
    };
    result.into()
}

#[proc_macro_attribute]
pub fn mutator(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let name = &input.ident;
    let result = quote! {
        #input

        #[cfg(not(target_os = "macos"))]
        mod __mallockit_mutator {
            #[thread_local]
            pub(super) static mut MUTATOR: super::#name = <super::#name as mallockit::Mutator>::NEW;
        }

        impl mallockit::mutator::TLS for #name {
            const NEW: Self = <Self as mallockit::Mutator>::NEW;

            #[cfg(not(target_os = "macos"))]
            #[inline(always)]
            fn current() -> &'static mut Self {
                unsafe { &mut crate::__mallockit_mutator::MUTATOR }
            }
        }
    };
    result.into()
}

#[proc_macro_attribute]
pub fn interpose(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let name = &input.sig.ident;
    let result = quote! {
        #[cfg(target_os = "macos")]
        pub mod #name {
            #[repr(C)]
            pub struct Interpose {
                _new: *const (),
                _old: *const (),
            }

            #[used]
            #[allow(non_upper_case_globals)]
            #[link_section = "__DATA,__interpose"]
            pub static mut interpose: Interpose = Interpose {
                _new: super::#name as *const (),
                _old: #name as *const (),
            };

            extern {
                pub fn #name();
            }
        }

        #[cfg(target_os = "macos")]
        #input

        #[cfg(not(target_os = "macos"))]
        #[no_mangle]
        #input
    };
    result.into()
}
