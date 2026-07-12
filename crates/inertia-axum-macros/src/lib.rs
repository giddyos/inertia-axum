//! Derives for typed inertia-axum pages and props.

use proc_macro::TokenStream;

mod attributes;
mod diagnostics;
mod form;
mod page;
mod props;
#[cfg(feature = "typegen")]
mod typegen;

#[proc_macro_derive(InertiaPage, attributes(inertia, ts))]
/// Derives a typed direct-response Inertia page and its prop keys.
pub fn derive_page(input: TokenStream) -> TokenStream {
    page::expand(syn::parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(InertiaProps, attributes(inertia, ts))]
/// Derives field-by-field conversion for typed shared or page props.
pub fn derive_props(input: TokenStream) -> TokenStream {
    props::expand(syn::parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[cfg(feature = "typegen")]
#[proc_macro_derive(InertiaType, attributes(inertia, serde, ts))]
/// Derives a test-only compiled TypeScript contract for an application type.
pub fn derive_inertia_type(input: TokenStream) -> TokenStream {
    typegen::expand(syn::parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}

#[proc_macro_derive(InertiaForm, attributes(inertia))]
/// Derives Inertia form validation metadata and adapters.
pub fn derive_form(input: TokenStream) -> TokenStream {
    form::expand(syn::parse_macro_input!(input))
        .unwrap_or_else(syn::Error::into_compile_error)
        .into()
}
