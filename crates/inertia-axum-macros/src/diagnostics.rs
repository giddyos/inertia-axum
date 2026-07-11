use proc_macro2::Span;

pub(crate) fn error(span: Span, message: impl std::fmt::Display) -> syn::Error {
    syn::Error::new(span, message)
}
