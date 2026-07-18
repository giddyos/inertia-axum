use crate::{diagnostics::error, props::runtime_path};
use proc_macro2::TokenStream;
use quote::{ToTokens, format_ident, quote};
use syn::{
    Attribute, Data, DeriveInput, Field, GenericParam, Meta, Token,
    ext::IdentExt,
    parse::{Parse, ParseStream, Parser},
    punctuated::Punctuated,
    spanned::Spanned,
};

pub(crate) enum RootFlavor {
    Page,
    Props,
}

pub(crate) fn expand_root(
    input: &DeriveInput,
    fields: &[crate::props::FieldInfo],
    runtime: &TokenStream,
    flavor: RootFlavor,
    component: Option<&syn::LitStr>,
    options: &crate::attributes::TypegenAttributes,
    shared: bool,
) -> syn::Result<TokenStream> {
    if options.skip {
        return Ok(TokenStream::new());
    }
    if !input.generics.params.is_empty() {
        return Err(error(
            input.generics.span(),
            "type-generation roots must be concrete; move generics into a nested InertiaType DTO",
        ));
    }
    let original = &input.ident;
    let prefix = match flavor {
        RootFlavor::Page => "__InertiaPageTypegen",
        RootFlavor::Props => "__InertiaPropsTypegen",
    };
    let proxy = format_ident!("{}{}", prefix, original);
    let default_name = match flavor {
        RootFlavor::Page => format!("{original}Props"),
        RootFlavor::Props => original.to_string(),
    };
    let ts_name = options
        .name
        .clone()
        .unwrap_or_else(|| syn::LitStr::new(&default_name, original.span()));
    let crate_path = format!(
        "{}::__private::typegen",
        runtime.to_string().replace(' ', "")
    );
    let crate_literal = syn::LitStr::new(&crate_path, original.span());
    let export_to = options
        .path
        .as_ref()
        .map_or_else(|| quote!(), |path| quote!(, export_to = #path));
    let doc_attributes = input
        .attrs
        .iter()
        .filter(|attribute| attribute.path().is_ident("doc"));
    let proxy_fields = fields
        .iter()
        .filter(|field| !field.skip)
        .map(|field| {
            let ident = &field.ident;
            let mut attributes = field.ts_attributes.clone();
            let rust_name = ident.to_string().trim_start_matches("r#").to_owned();
            if field.serialized_name != rust_name && !has_ts_option(&attributes, "rename")? {
                let rename = syn::LitStr::new(&field.serialized_name, ident.span());
                attributes.push(syn::parse_quote!(#[ts(rename = #rename)]));
            }
            let ty = &field.exported_ty;
            if field.is_prop {
                if !has_ts_option(&attributes, "optional")? {
                    attributes.push(syn::parse_quote!(#[ts(optional)]));
                }
                Ok(quote!(#(#attributes)* #ident: Option<#ty>))
            } else {
                Ok(quote!(#(#attributes)* #ident: #ty))
            }
        })
        .collect::<syn::Result<Vec<_>>>()?;
    let kind = match flavor {
        RootFlavor::Page => quote!(#runtime::__private::typegen::RootKind::Page),
        RootFlavor::Props => quote!(#runtime::__private::typegen::RootKind::Props),
    };
    let component_value = component.map_or_else(|| quote!(None), |value| quote!(Some(#value)));
    let test = format_ident!(
        "__inertia_typegen_export_{}_{}",
        match flavor {
            RootFlavor::Page => "page",
            RootFlavor::Props => "props",
        },
        to_snake_case(&original.to_string())
    );
    Ok(quote! {
        #[cfg(test)]
        #(#doc_attributes)*
        #[derive(#runtime::__private::typegen::TS)]
        #[ts(crate = #crate_literal, rename = #ts_name #export_to)]
        struct #proxy { #(#proxy_fields,)* }

        #[cfg(test)]
        #[test]
        fn #test() {
            if std::env::var_os("INERTIA_TYPEGEN_STAGING").is_none() { return; }
            #runtime::__private::typegen::export_root::<#proxy>(
                #runtime::__private::typegen::RootMetadata {
                    kind: #kind,
                    rust_name: stringify!(#original),
                    ts_name: #ts_name,
                    component: #component_value,
                    shared: #shared,
                    source: #runtime::__private::typegen::SourceLocation {
                        file: file!().into(), line: line!(), module: module_path!().into(),
                    },
                },
            ).expect("failed to export Inertia root");
        }
    })
}

pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    if matches!(input.data, Data::Union(_)) {
        return Err(error(input.span(), "InertiaType does not support unions"));
    }
    validate_fields(&input)?;
    let runtime = runtime_path()?;
    let original = &input.ident;
    let proxy = format_ident!("__InertiaTypeProxy{}", original);
    let test = format_ident!(
        "__inertia_typegen_metadata_{}",
        to_snake_case(&original.to_string())
    );
    let mut mirror = input.clone();
    mirror.ident = proxy.clone();
    mirror.vis = input.vis.clone();
    mirror.attrs = forwarded_attributes(&input.attrs, true)?;
    mirror.attrs.insert(
        0,
        syn::parse_quote!(
        #[derive(
            #runtime::__private::typegen::serde::Serialize,
            #runtime::__private::typegen::TS
        )]
        ),
    );
    let crate_path = format!(
        "{}::__private::typegen",
        runtime.to_string().replace(' ', "")
    );
    let has_rename = has_ts_option(&input.attrs, "rename")?;
    let crate_literal = syn::LitStr::new(&crate_path, original.span());
    if has_rename {
        mirror
            .attrs
            .insert(1, syn::parse_quote!(#[ts(crate = #crate_literal)]));
    } else {
        let name = syn::LitStr::new(&original.to_string(), original.span());
        mirror.attrs.insert(
            1,
            syn::parse_quote!(#[ts(crate = #crate_literal, rename = #name)]),
        );
    }
    clean_data_attributes(&mut mirror.data)?;

    let mut impl_generics_source = input.generics.clone();
    for parameter in &input.generics.params {
        if let GenericParam::Type(parameter) = parameter {
            let ident = &parameter.ident;
            let predicate: syn::WherePredicate =
                syn::parse2(quote!(#ident: #runtime::__private::typegen::TS + 'static))?;
            impl_generics_source
                .make_where_clause()
                .predicates
                .push(predicate);
        }
    }
    let (impl_generics, _, where_clause) = impl_generics_source.split_for_impl();
    let (_, ty_generics, _) = input.generics.split_for_impl();
    let ts = quote!(#runtime::__private::typegen::TS);

    let metadata_test = input.generics.params.is_empty().then(|| {
        quote! {
        #[cfg(test)]
        #[test]
        fn #test() {
            if std::env::var_os("INERTIA_TYPEGEN_STAGING").is_none() { return; }
            #runtime::__private::typegen::export_supporting_type::<#original>(
                    #runtime::__private::typegen::TypeMetadata {
                        rust_name: stringify!(#original),
                        source: #runtime::__private::typegen::SourceLocation {
                            file: file!().into(), line: line!(), module: module_path!().into(),
                        },
                    },
                ).expect("failed to export InertiaType");
            }
        }
    });

    Ok(quote! {
        #[cfg(test)]
        #mirror

        #[cfg(test)]
        impl #impl_generics #ts for #original #ty_generics #where_clause {
            type WithoutGenerics = <#proxy #ty_generics as #ts>::WithoutGenerics;
            type OptionInnerType = Self;
            const IS_OPTION: bool = <#proxy #ty_generics as #ts>::IS_OPTION;
            const IS_ENUM: bool = <#proxy #ty_generics as #ts>::IS_ENUM;
            fn docs() -> Option<String> { <#proxy #ty_generics as #ts>::docs() }
            fn ident(config: &#runtime::__private::typegen::Config) -> String { <#proxy #ty_generics as #ts>::ident(config) }
            fn name(config: &#runtime::__private::typegen::Config) -> String { <#proxy #ty_generics as #ts>::name(config) }
            fn inline(config: &#runtime::__private::typegen::Config) -> String { <#proxy #ty_generics as #ts>::inline(config) }
            fn inline_flattened(config: &#runtime::__private::typegen::Config) -> String { <#proxy #ty_generics as #ts>::inline_flattened(config) }
            fn visit_dependencies(visitor: &mut impl #runtime::__private::typegen::TypeVisitor) where Self: 'static { <#proxy #ty_generics as #ts>::visit_dependencies(visitor); }
            fn visit_generics(visitor: &mut impl #runtime::__private::typegen::TypeVisitor) where Self: 'static { <#proxy #ty_generics as #ts>::visit_generics(visitor); }
            fn decl(config: &#runtime::__private::typegen::Config) -> String { <#proxy #ty_generics as #ts>::decl(config) }
            fn decl_concrete(config: &#runtime::__private::typegen::Config) -> String { <#proxy #ty_generics as #ts>::decl_concrete(config) }
            fn output_path() -> Option<std::path::PathBuf> { <#proxy #ty_generics as #ts>::output_path() }
        }

        #metadata_test
    })
}

fn clean_data_attributes(data: &mut Data) -> syn::Result<()> {
    let fields: Box<dyn Iterator<Item = &mut Field> + '_> = match data {
        Data::Struct(data) => Box::new(data.fields.iter_mut()),
        Data::Enum(data) => Box::new(
            data.variants
                .iter_mut()
                .flat_map(|variant| variant.fields.iter_mut()),
        ),
        Data::Union(_) => unreachable!(),
    };
    for field in fields {
        field.attrs = forwarded_attributes(&field.attrs, false)?;
    }
    if let Data::Enum(data) = data {
        for variant in &mut data.variants {
            variant.attrs = forwarded_attributes(&variant.attrs, false)?;
        }
    }
    Ok(())
}

fn forwarded_attributes(attributes: &[Attribute], container: bool) -> syn::Result<Vec<Attribute>> {
    let mut output = Vec::new();
    for attribute in attributes {
        if attribute.path().is_ident("doc") || attribute.path().is_ident("serde") {
            output.push(attribute.clone());
        } else if attribute.path().is_ident("ts") {
            let options = parse_ts_options(attribute)?
                .into_iter()
                .filter(|option| option.name != "export" && option.name != "crate")
                .collect::<Punctuated<TsOption, Token![,]>>();
            if !options.is_empty() {
                output.push(syn::parse_quote!(#[ts(#options)]));
            }
        } else if !container && attribute.path().is_ident("cfg") {
            output.push(attribute.clone());
        }
    }
    Ok(output)
}

fn validate_fields(input: &DeriveInput) -> syn::Result<()> {
    let fields: Vec<&Field> = match &input.data {
        Data::Struct(data) => data.fields.iter().collect(),
        Data::Enum(data) => data
            .variants
            .iter()
            .flat_map(|variant| variant.fields.iter())
            .collect(),
        Data::Union(_) => return Ok(()),
    };
    for field in fields {
        let custom = has_serde_option(&field.attrs, "serialize_with")?
            || has_serde_option(&field.attrs, "with")?;
        if custom && !has_ts_option(&field.attrs, "type")? && !has_ts_option(&field.attrs, "as")? {
            let field_name = field
                .ident
                .as_ref()
                .map_or_else(|| "tuple field".into(), ToString::to_string);
            return Err(error(
                field.span(),
                format!(
                    "error[INERTIA-TYPEGEN-010]: `{field_name}` uses custom Serde serialization but does not declare its TypeScript wire representation; add #[ts(type = \"...\")] or #[ts(as = \"...\")]"
                ),
            ));
        }
        if has_serde_option(&field.attrs, "skip_serializing_if")?
            && !has_ts_option(&field.attrs, "optional")?
            && !is_option(&field.ty)
        {
            return Err(error(
                field.span(),
                "error[INERTIA-TYPEGEN-011]: skip_serializing_if requires Option<T> or #[ts(optional)] so the TypeScript property can be absent",
            ));
        }
    }
    Ok(())
}

fn parse_options(attribute: &Attribute) -> syn::Result<Punctuated<Meta, Token![,]>> {
    match &attribute.meta {
        Meta::List(list) => {
            Punctuated::<Meta, Token![,]>::parse_terminated.parse2(list.tokens.clone())
        }
        _ => Err(error(
            attribute.span(),
            "expected a parenthesized attribute",
        )),
    }
}

fn has_ts_option(attributes: &[Attribute], name: &str) -> syn::Result<bool> {
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("ts"))
    {
        if parse_ts_options(attribute)?
            .iter()
            .any(|option| option.name == name)
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn has_serde_option(attributes: &[Attribute], name: &str) -> syn::Result<bool> {
    has_option(attributes, "serde", name)
}

struct TsOption {
    name: syn::Ident,
    suffix: TokenStream,
}

impl Parse for TsOption {
    fn parse(input: ParseStream<'_>) -> syn::Result<Self> {
        let name = syn::Ident::parse_any(input)?;
        let suffix = if input.peek(Token![=]) {
            let equals: Token![=] = input.parse()?;
            let value: syn::Expr = input.parse()?;
            quote!(#equals #value)
        } else if input.peek(syn::token::Paren) {
            let content;
            let _paren = syn::parenthesized!(content in input);
            let inner: TokenStream = content.parse()?;
            quote!((#inner))
        } else {
            TokenStream::new()
        };
        Ok(Self { name, suffix })
    }
}

impl ToTokens for TsOption {
    fn to_tokens(&self, tokens: &mut TokenStream) {
        self.name.to_tokens(tokens);
        self.suffix.to_tokens(tokens);
    }
}

fn parse_ts_options(attribute: &Attribute) -> syn::Result<Punctuated<TsOption, Token![,]>> {
    match &attribute.meta {
        Meta::List(list) => {
            Punctuated::<TsOption, Token![,]>::parse_terminated.parse2(list.tokens.clone())
        }
        _ => Err(error(
            attribute.span(),
            "expected a parenthesized attribute",
        )),
    }
}

fn has_option(attributes: &[Attribute], attribute_name: &str, name: &str) -> syn::Result<bool> {
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident(attribute_name))
    {
        if parse_options(attribute)?
            .iter()
            .any(|meta| meta.path().is_ident(name))
        {
            return Ok(true);
        }
    }
    Ok(false)
}

fn is_option(ty: &syn::Type) -> bool {
    matches!(ty, syn::Type::Path(path) if path.path.segments.last().is_some_and(|segment| segment.ident == "Option"))
}

fn to_snake_case(value: &str) -> String {
    let mut output = String::new();
    for (index, character) in value.chars().enumerate() {
        if character.is_uppercase() && index > 0 {
            output.push('_');
        }
        output.extend(character.to_lowercase());
    }
    output
}
