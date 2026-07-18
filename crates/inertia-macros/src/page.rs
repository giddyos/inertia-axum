use crate::{attributes, diagnostics::error, props};
use proc_macro2::TokenStream;
use quote::quote;
use syn::{DeriveInput, spanned::Spanned};

pub(crate) fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let container = attributes::container(&input.attrs)?;
    let component = container.component.ok_or_else(|| {
        error(
            input.span(),
            "InertiaPage requires #[inertia(component = \"...\")]",
        )
    })?;
    if component.value().trim().is_empty() {
        return Err(error(
            component.span(),
            "Inertia component names cannot be empty",
        ));
    }
    let runtime = props::runtime_path()?;
    if container.shared {
        return Err(error(
            input.span(),
            "#[inertia(shared)] is valid only on InertiaProps",
        ));
    }
    #[cfg(feature = "typegen")]
    if !input.generics.params.is_empty() && !container.typegen.skip {
        return Err(error(
            input.generics.span(),
            "error[INERTIA-TYPEGEN-012]: generic Inertia page roots require a concrete frontend contract; use a concrete page type",
        ));
    }
    let fields = props::fields(&input, container.rename_all)?;
    let props_impl = props::props_impl(&input, &fields, &runtime);
    let constants = props::key_constants(&fields, &runtime);
    let name = &input.ident;
    let generics = props::add_self_bound(input.generics.clone());
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();
    let encrypt = container
        .encrypt_history
        .then(|| quote!(.encrypt_history()));
    let clear = container.clear_history.then(|| quote!(.clear_history()));
    let preserve = container
        .preserve_fragment
        .then(|| quote!(.preserve_fragment()));
    #[cfg(feature = "typegen")]
    let exporter = crate::typegen::expand_root(
        &input,
        &fields,
        &runtime,
        crate::typegen::RootFlavor::Page,
        Some(&component),
        &container.typegen,
        false,
    )?;
    #[cfg(not(feature = "typegen"))]
    let exporter = TokenStream::new();
    Ok(quote! {
        #props_impl
        impl #impl_generics #name #ty_generics #where_clause {
            pub const COMPONENT: #runtime::Component = #runtime::Component::new(#component);
            #(#constants)*
        }
        impl #impl_generics #runtime::InertiaPage for #name #ty_generics #where_clause {
            const COMPONENT: #runtime::Component = #runtime::Component::new(#component);
            fn into_pending_page(self) -> #runtime::PendingPage {
                let props = #runtime::IntoInertiaProps::into_inertia_props(self);
                #runtime::PendingPage::from_typed(Self::COMPONENT, props, Self::options())
            }
            fn options() -> #runtime::PageOptions {
                #runtime::PageOptions::new() #encrypt #clear #preserve
            }
        }
        #exporter
    })
}
