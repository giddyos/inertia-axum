use crate::{attributes, diagnostics::error};
use proc_macro2::{Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::{format_ident, quote};
use syn::{parse_quote, spanned::Spanned, Data, DeriveInput, Fields, Generics, Ident, Type};

pub(crate) struct FieldInfo {
    pub ident: Ident,
    pub key_ty: Type,
    pub serialized_name: String,
    pub skip: bool,
}

pub(crate) fn runtime_path() -> syn::Result<TokenStream> {
    match crate_name("inertia-axum").map_err(|error| syn::Error::new(Span::call_site(), error))? {
        FoundCrate::Itself => Ok(quote!(crate)),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            Ok(quote!(::#ident))
        }
    }
}

pub(crate) fn fields(
    input: &DeriveInput,
    rename_all: Option<attributes::RenameRule>,
) -> syn::Result<Vec<FieldInfo>> {
    let Data::Struct(data) = &input.data else {
        return Err(error(
            input.span(),
            "Inertia derives support structs with named fields only",
        ));
    };
    let Fields::Named(fields) = &data.fields else {
        return Err(error(
            data.fields.span(),
            "Inertia derives support structs with named fields only",
        ));
    };
    fields
        .named
        .iter()
        .map(|field| {
            let ident = field.ident.clone().expect("named field");
            let attrs = attributes::field(&field.attrs)?;
            let rust_name = ident.to_string().trim_start_matches("r#").to_owned();
            let serialized_name = attrs.rename.map_or_else(
                || rename_all.map_or(rust_name.clone(), |rule| rule.apply(&rust_name)),
                |value| value.value(),
            );
            if !attrs.skip && serialized_name == "errors" {
                return Err(error(
                    field.span(),
                    "\"errors\" is reserved by the Inertia validation protocol",
                ));
            }
            let key_ty = prop_inner(&field.ty).unwrap_or_else(|| field.ty.clone());
            Ok(FieldInfo {
                ident,
                key_ty,
                serialized_name,
                skip: attrs.skip,
            })
        })
        .collect()
}

fn prop_inner(ty: &Type) -> Option<Type> {
    let Type::Path(path) = ty else {
        return None;
    };
    let segment = path.path.segments.last()?;
    if segment.ident != "Prop" {
        return None;
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };
    arguments.args.iter().find_map(|argument| {
        if let syn::GenericArgument::Type(ty) = argument {
            Some(ty.clone())
        } else {
            None
        }
    })
}

pub(crate) fn add_self_bound(mut generics: Generics) -> Generics {
    generics
        .make_where_clause()
        .predicates
        .push(parse_quote!(Self: Send + 'static));
    generics
}

pub(crate) fn props_impl(
    input: &DeriveInput,
    fields: &[FieldInfo],
    runtime: &TokenStream,
) -> TokenStream {
    let name = &input.ident;
    let generics = add_self_bound(input.generics.clone());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let bindings = fields.iter().map(|field| {
        let ident = &field.ident;
        if field.skip {
            quote!(#ident: _)
        } else {
            quote!(#ident)
        }
    });
    let pushes = fields.iter().filter(|field| !field.skip).map(|field| {
        let ident = &field.ident;
        let serialized = &field.serialized_name;
        quote! {{
            use #runtime::__private::IntoPendingProp as _;
            let mut adapter = #runtime::__private::DynamicPropAdapter::new(#ident);
            props.push(adapter.into_pending_prop(#serialized.to_owned()));
        }}
    });
    quote! {
        impl #impl_generics #runtime::IntoInertiaProps for #name #ty_generics #where_clause {
            fn into_inertia_props(self) -> #runtime::Props {
                let Self { #(#bindings),* } = self;
                let mut props = #runtime::Props::new();
                #(#pushes)*
                props
            }
        }
    }
}

pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let container = attributes::container(&input.attrs)?;
    if container.component.is_some()
        || container.encrypt_history
        || container.clear_history
        || container.preserve_fragment
    {
        return Err(error(
            input.span(),
            "InertiaProps supports only the rename_all container attribute",
        ));
    }
    let runtime = runtime_path()?;
    let fields = fields(&input, container.rename_all)?;
    Ok(props_impl(&input, &fields, &runtime))
}

pub(crate) fn key_constants(fields: &[FieldInfo], runtime: &TokenStream) -> Vec<TokenStream> {
    fields.iter().filter(|field| !field.skip).map(|field| {
        let constant = format_ident!("{}", field.ident.to_string().trim_start_matches("r#").to_uppercase());
        let ty = &field.key_ty;
        let serialized = &field.serialized_name;
        quote! { pub const #constant: #runtime::PropKey<#ty> = #runtime::PropKey::new(Self::COMPONENT, #serialized); }
    }).collect()
}
