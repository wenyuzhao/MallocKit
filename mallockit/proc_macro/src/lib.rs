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

            mallockit::export_malloc_api!(PLAN, super::super::#name);
        }

        impl mallockit::plan::Singleton for #name {
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
        #[repr(align(256))]
        #input

        #[cfg(not(target_os = "macos"))]
        mod __mallockit_mutator {
            #[thread_local]
            pub(super) static mut MUTATOR: mallockit::util::Lazy<super::#name, mallockit::util::Local> = mallockit::util::Lazy::new(|| <super::#name as mallockit::Mutator>::new());
        }

        impl mallockit::mutator::TLS for #name {
            fn new() -> Self {
                <Self as mallockit::Mutator>::new()
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
