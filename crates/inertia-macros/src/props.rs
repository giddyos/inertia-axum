use crate::{attributes, diagnostics::error};
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
#[cfg(feature = "typegen")]
use syn::Attribute;
use syn::{Data, DeriveInput, Fields, Generics, Ident, Type, parse_quote, spanned::Spanned};

pub(crate) struct FieldInfo {
    pub ident: Ident,
    #[allow(dead_code)]
    pub rust_ty: Type,
    #[cfg(feature = "typegen")]
    pub exported_ty: Type,
    pub key_ty: Type,
    pub serialized_name: String,
    pub skip: bool,
    #[cfg(feature = "typegen")]
    pub is_prop: bool,
    #[cfg(feature = "typegen")]
    pub ts_attributes: Vec<Attribute>,
}

pub(crate) fn runtime_path() -> syn::Result<TokenStream> {
    for (candidate, is_core) in [
        ("inertia-core", true),
        ("inertia-axum", false),
        ("inertia-actix", false),
        ("inertia-rocket", false),
    ] {
        if let Ok(found) = crate_name(candidate) {
            let runtime = match found {
                FoundCrate::Itself => quote!(crate),
                FoundCrate::Name(name) => {
                    let ident = Ident::new(&name, Span::call_site());
                    quote!(::#ident)
                }
            };
            return if is_core {
                Ok(runtime)
            } else {
                Ok(quote!(#runtime::__private::core))
            };
        }
    }

    Err(syn::Error::new(
        Span::call_site(),
        "Inertia derives require inertia-core or a supported framework adapter dependency",
    ))
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
            let prop = prop_inner(&field.ty);
            let key_ty = prop.clone().unwrap_or_else(|| field.ty.clone());
            Ok(FieldInfo {
                ident,
                rust_ty: field.ty.clone(),
                #[cfg(feature = "typegen")]
                exported_ty: key_ty.clone(),
                key_ty,
                serialized_name,
                skip: attrs.skip,
                #[cfg(feature = "typegen")]
                is_prop: prop.is_some(),
                #[cfg(feature = "typegen")]
                ts_attributes: field
                    .attrs
                    .iter()
                    .filter(|attribute| attribute.path().is_ident("ts"))
                    .cloned()
                    .collect(),
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
        || container.shared && container.typegen.skip
    {
        return Err(error(
            input.span(),
            "invalid InertiaProps container attribute combination",
        ));
    }
    let runtime = runtime_path()?;
    let fields = fields(&input, container.rename_all)?;
    let runtime_impl = props_impl(&input, &fields, &runtime);
    #[cfg(feature = "typegen")]
    let exporter = crate::typegen::expand_root(
        &input,
        &fields,
        &runtime,
        crate::typegen::RootFlavor::Props,
        None,
        &container.typegen,
        container.shared,
    )?;
    #[cfg(not(feature = "typegen"))]
    let exporter = TokenStream::new();
    Ok(quote!(#runtime_impl #exporter))
}

pub(crate) fn key_constants(fields: &[FieldInfo], runtime: &TokenStream) -> Vec<TokenStream> {
    fields.iter().filter(|field| !field.skip).map(|field| {
        let constant = format_ident!("{}", field.ident.to_string().trim_start_matches("r#").to_uppercase());
        let ty = &field.key_ty;
        let serialized = &field.serialized_name;
        quote! { pub const #constant: #runtime::PropKey<#ty> = #runtime::PropKey::new(Self::COMPONENT, #serialized); }
    }).collect()
}
