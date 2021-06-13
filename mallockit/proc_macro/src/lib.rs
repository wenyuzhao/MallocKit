use proc_macro::TokenStream;
use quote::quote;

#[proc_macro_attribute]
pub fn plan(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemStatic);
    let name = &input.ident;
    let result = quote! {
        #input
        mallockit::export_malloc_api!(#name);
    };
    result.into()
}

#[proc_macro_attribute]
pub fn mutator(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let name = &input.ident;
    let result = quote! {
        #input

        mod __mallockit_mutator {
            #[thread_local]
            #[cfg(not(target_os = "macos"))]
            pub(crate) static mut MUTATOR: super::#name = <super::#name as mallockit::Mutator>::NEW;
        }

        impl mallockit::thread_local::TLS for #name {
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

#[proc_macro_attribute]
pub fn malloc_test(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::ItemFn);
    let name = &input.sig.ident;
    let result = quote! {
        #input
        crate::test_all_malloc!(#name);
    };
    result.into()
}
