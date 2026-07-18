use crate::build::{BuiltAssetBytes, BuiltFrontend, BuiltStorage};
use proc_macro_crate::{FoundCrate, crate_name};
use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{Ident, LitBool, LitByteStr, LitStr};

pub(crate) fn expand(frontend: BuiltFrontend) -> syn::Result<TokenStream> {
    let runtime = runtime_path()?;
    let public_path = LitStr::new(&frontend.public_path, Span::call_site());
    let entry = LitStr::new(&frontend.entry, Span::call_site());
    let version = LitStr::new(&frontend.version, Span::call_site());
    let tags = LitStr::new(&frontend.tags, Span::call_site());
    let manifest = LitStr::new(&frontend.manifest.to_string_lossy(), Span::call_site());
    let assets = frontend.assets.iter().map(|asset| {
        let path = LitStr::new(&asset.path, Span::call_site());
        let bytes = match &asset.bytes {
            BuiltAssetBytes::File(absolute) => {
                let absolute = LitStr::new(&absolute.to_string_lossy(), Span::call_site());
                quote!(include_bytes!(#absolute))
            }
            BuiltAssetBytes::Generated { bytes, source } => {
                let bytes = LitByteStr::new(bytes, Span::call_site());
                let source = LitStr::new(&source.to_string_lossy(), Span::call_site());
                quote!({
                    const _: &[u8] = include_bytes!(#source);
                    #bytes
                })
            }
        };
        let content_type = LitStr::new(&asset.content_type, Span::call_site());
        let etag = LitStr::new(&asset.etag, Span::call_site());
        let immutable = LitBool::new(asset.immutable, Span::call_site());
        let storage = match asset.storage {
            BuiltStorage::Identity => quote!(#runtime::EmbeddedStorage::Identity),
            BuiltStorage::Brotli { uncompressed_len } => {
                quote!(#runtime::EmbeddedStorage::Brotli {
                    uncompressed_len: #uncompressed_len,
                })
            }
        };
        quote! {
            #runtime::EmbeddedAsset {
                path: #path,
                bytes: #bytes,
                storage: #storage,
                content_type: #content_type,
                etag: #etag,
                immutable: #immutable,
                encoding: None,
            }
        }
    });
    Ok(quote! {{
        const _: &str = include_str!(#manifest);
        const ASSETS: &[#runtime::EmbeddedAsset] = &[#(#assets),*];
        #runtime::EmbeddedFrontend::new(
            #public_path,
            #entry,
            #version,
            #tags,
            ASSETS,
        )
    }})
}

fn runtime_path() -> syn::Result<TokenStream> {
    let found = crate_name("inertia-embed").map_err(|_| {
        syn::Error::new(
            Span::call_site(),
            "embed_frontend! requires an inertia-embed dependency",
        )
    })?;
    Ok(match found {
        FoundCrate::Itself => quote!(crate),
        FoundCrate::Name(name) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
    })
}
