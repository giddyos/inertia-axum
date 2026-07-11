//! Derives for typed inertia-axum pages and props.

use proc_macro::TokenStream;

mod attributes;
mod diagnostics;
mod form;
mod page;
mod props;

#[proc_macro_derive(InertiaPage, attributes(inertia))]
pub fn derive_page(input: TokenStream) -> TokenStream {
    page::expand(syn::parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(InertiaProps, attributes(inertia))]
pub fn derive_props(input: TokenStream) -> TokenStream {
    props::expand(syn::parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
