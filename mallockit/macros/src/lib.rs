use std::collections::HashMap;

use proc_macro::TokenStream;
use quote::quote;

fn construct_tests_from_config() -> Vec<proc_macro2::TokenStream> {
    let meta = cargo_metadata::MetadataCommand::new().exec().unwrap();
    let mut tests = HashMap::new();
    let ws_meta = meta.workspace_metadata.as_object();
    if let Some(v) = ws_meta.and_then(|v| v.get("malloc-tests")) {
        for (name, cmd) in v.as_object().unwrap() {
            let cmd = cmd.as_str().unwrap();
            tests.insert(name.to_owned(), cmd.to_owned());
        }
    }
    tests
        .iter()
        .map(|(k, v)| {
            let s = format!(
                r#"
                    #[test]
                    fn {}() {{
                        ::mallockit::util::testing::malloc::test(env!("CARGO_CRATE_NAME"), {:?});
                    }}
                "#,
                k, v,
            );
            s.parse().unwrap()
        })
        .collect()
}

#[proc_macro_attribute]
pub fn plan(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = syn::parse_macro_input!(item as syn::DeriveInput);
    let name = &input.ident;

    let tests = construct_tests_from_config();

    let result = quote! {
        #input

        mod __mallockit_plan {
            type Plan = super::#name;

            static PLAN: ::mallockit::util::Lazy<Plan> = ::mallockit::util::Lazy::new(|| <Plan as ::mallockit::Plan>::new());

            #[cfg(feature = "malloc")]
            #[::mallockit::ctor]
            unsafe fn ctor() {
                <<Plan as ::mallockit::Plan>::Mutator as ::mallockit::mutator::TLS>::current();
                ::mallockit::util::sys::hooks::process_start(&*PLAN);
            }

            #[cfg(target_os = "macos")]
            #[no_mangle]
            extern "C" fn mallockit_initialize_macos_tls() -> *mut u8 {
                use ::mallockit::mutator::TLS;
                <Plan as ::mallockit::Plan>::Mutator::current() as *mut <Plan as ::mallockit::Plan>::Mutator as _
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
            #[cfg(feature = "malloc")]
            mod malloc {
                #(#tests)*
            }
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


        mod __mallockit_mutator {
            #[cfg(not(target_os = "macos"))]
            fn init() -> super::#name {
                ::mallockit::mutator::init_pthread_specific();
                <super::#name as ::mallockit::Mutator>::new()
            }

            #[cfg(not(target_os = "macos"))]
            #[thread_local]
            pub(super) static mut MUTATOR: ::mallockit::util::Lazy<super::#name, ::mallockit::util::Local> = ::mallockit::util::Lazy::new(init);

            #[no_mangle]
            #[cfg(not(target_os = "macos"))]
            extern "C" fn mallockit_pthread_destructor() {
                unsafe {
                    MUTATOR.reset(init);
                }
            }

            #[no_mangle]
            #[cfg(target_os = "macos")]
            extern "C" fn mallockit_pthread_destructor() {
                use crate::mallockit::mutator::TLS;
                <super::#name as ::mallockit::mutator::TLS>::current().reset();
            }
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
