use crate::{diagnostics::error, props};
use proc_macro2::TokenStream;
use quote::quote;
use std::collections::BTreeSet;
use syn::{Data, DeriveInput, Fields, LitStr, Path, spanned::Spanned};

enum ValidationBackend {
    Garde,
    ValidatorCrate,
    Custom(Path),
    None,
}

pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let mut validator = ValidationBackend::None;
    let mut validator_set = false;
    let mut error_bag = None::<LitStr>;
    let mut old_input = false;
    let mut redacted = BTreeSet::new();
    for attribute in input
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("inertia"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("validator") {
                if validator_set {
                    return Err(meta.error("only one inertia validator may be configured"));
                }
                validator_set = true;
                let value: LitStr = meta.value()?.parse()?;
                validator = match value.value().as_str() {
                    "garde" => ValidationBackend::Garde,
                    "validator" => ValidationBackend::ValidatorCrate,
                    _ => {
                        return Err(error(
                            value.span(),
                            "invalid inertia validator; expected \"garde\" or \"validator\"",
                        ));
                    }
                };
            } else if meta.path.is_ident("validate_with") {
                if validator_set {
                    return Err(meta.error("only one inertia validator may be configured"));
                }
                validator_set = true;
                let value: LitStr = meta.value()?.parse()?;
                validator = ValidationBackend::Custom(value.parse()?);
            } else if meta.path.is_ident("error_bag") {
                if error_bag.is_some() {
                    return Err(meta.error("duplicate inertia error_bag"));
                }
                error_bag = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("old_input") {
                if old_input {
                    return Err(meta.error("duplicate inertia old_input"));
                }
                old_input = true;
            } else if meta.path.is_ident("redact") {
                let value: LitStr = meta.value()?.parse()?;
                for name in value
                    .value()
                    .split(',')
                    .map(str::trim)
                    .filter(|name| !name.is_empty())
                {
                    redacted.insert(name.to_owned());
                }
            } else {
                return Err(meta.error("unsupported inertia form attribute"));
            }
            Ok(())
        })?;
    }
    let Data::Struct(data) = &input.data else {
        return Err(error(
            input.span(),
            "InertiaForm supports structs with named fields only",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(error(
            data.fields.span(),
            "InertiaForm supports structs with named fields only",
        ));
    };
    for field in &fields.named {
        let ident = field.ident.as_ref().expect("named field");
        for attribute in field
            .attrs
            .iter()
            .filter(|attribute| attribute.path().is_ident("inertia"))
        {
            attribute.parse_nested_meta(|meta| {
                if meta.path.is_ident("redact") || meta.path.is_ident("sensitive") {
                    redacted.insert(ident.to_string().trim_start_matches("r#").to_owned());
                    Ok(())
                } else {
                    Err(meta.error("unsupported inertia form field attribute"))
                }
            })?;
        }
    }
    for name in &redacted {
        if !fields.named.iter().any(|field| {
            field
                .ident
                .as_ref()
                .is_some_and(|ident| ident.to_string().trim_start_matches("r#") == name)
        }) {
            return Err(error(
                input.span(),
                format!("inertia redact field \"{name}\" does not exist"),
            ));
        }
    }
    let runtime = props::runtime_path()?;
    let name = &input.ident;
    let generics = props::add_self_bound(input.generics.clone());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let validation = match validator {
        ValidationBackend::Garde => {
            quote! { match ::garde::Validate::validate(self) { Ok(()) => Ok(()), Err(report) => { let mut errors = #runtime::Errors::new(); for (path, error) in report.iter() { errors.add(path.to_string(), error.to_string()); } Err(errors) } } }
        }
        ValidationBackend::ValidatorCrate => {
            quote! { match ::validator::Validate::validate(self) { Ok(()) => Ok(()), Err(report) => { let mut errors = #runtime::Errors::new(); for (field, field_errors) in report.field_errors() { if let Some(error) = field_errors.first() { errors.add(field, error.message.as_deref().unwrap_or(error.code.as_ref())); } } Err(errors) } } }
        }
        ValidationBackend::Custom(path) => quote!(#path(self)),
        ValidationBackend::None => quote!(Ok(())),
    };
    let bag = error_bag.map_or_else(|| quote!(None), |bag| quote!(Some(#bag)));
    let old_input_impl = if old_input {
        let values = fields.named.iter().filter_map(|field| {
            let ident = field.ident.as_ref()?;
            let rust_name = ident.to_string().trim_start_matches("r#").to_owned();
            (!redacted.contains(&rust_name))
                .then(|| quote!((#rust_name, #runtime::__private::to_value(&self.#ident))))
        });
        quote!(Some(#runtime::form::serialize_old_input([#(#values),*])))
    } else {
        quote!(None)
    };
    Ok(quote! {
        impl #impl_generics #runtime::Validate for #name #ty_generics #where_clause {
            fn validate(&self) -> Result<(), #runtime::Errors> { #validation }
            fn error_bag() -> Option<&'static str> { #bag }
            fn old_input(&self) -> Option<#runtime::__private::Value> { #old_input_impl }
        }
    })
}
