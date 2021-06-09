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
