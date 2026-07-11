use crate::diagnostics::error;
use syn::{spanned::Spanned, Attribute, LitStr};

#[derive(Default)]
pub(crate) struct ContainerAttributes {
    pub component: Option<LitStr>,
    pub rename_all: Option<RenameRule>,
    pub encrypt_history: bool,
    pub clear_history: bool,
    pub preserve_fragment: bool,
}

#[derive(Clone, Copy)]
pub(crate) enum RenameRule {
    Camel,
    Snake,
    Kebab,
    Pascal,
}

impl RenameRule {
    fn parse(value: &LitStr) -> syn::Result<Self> {
        match value.value().as_str() {
            "camelCase" => Ok(Self::Camel),
            "snake_case" => Ok(Self::Snake),
            "kebab-case" => Ok(Self::Kebab),
            "PascalCase" => Ok(Self::Pascal),
            _ => Err(error(value.span(), "unsupported inertia rename_all rule; expected camelCase, snake_case, kebab-case, or PascalCase")),
        }
    }
    pub fn apply(self, value: &str) -> String {
        let words = words(value);
        match self {
            Self::Snake => words.join("_"),
            Self::Kebab => words.join("-"),
            Self::Camel => words
                .iter()
                .enumerate()
                .map(|(index, word)| {
                    if index == 0 {
                        word.clone()
                    } else {
                        capitalize(word)
                    }
                })
                .collect(),
            Self::Pascal => words.iter().map(|word| capitalize(word)).collect(),
        }
    }
}

fn words(value: &str) -> Vec<String> {
    value
        .trim_start_matches("r#")
        .split('_')
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect()
}
fn capitalize(value: &str) -> String {
    let mut chars = value.chars();
    chars.next().map_or_else(String::new, |first| {
        first.to_uppercase().collect::<String>() + chars.as_str()
    })
}

pub(crate) fn container(attributes: &[Attribute]) -> syn::Result<ContainerAttributes> {
    let mut output = ContainerAttributes::default();
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("inertia"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("component") {
                if output.component.is_some() {
                    return Err(meta.error("duplicate inertia component"));
                }
                output.component = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("rename_all") {
                if output.rename_all.is_some() {
                    return Err(meta.error("duplicate inertia rename_all"));
                }
                output.rename_all = Some(RenameRule::parse(&meta.value()?.parse::<LitStr>()?)?);
            } else if meta.path.is_ident("encrypt_history") {
                output.encrypt_history =
                    set_flag(output.encrypt_history, &meta, "encrypt_history")?;
            } else if meta.path.is_ident("clear_history") {
                output.clear_history = set_flag(output.clear_history, &meta, "clear_history")?;
            } else if meta.path.is_ident("preserve_fragment") {
                output.preserve_fragment =
                    set_flag(output.preserve_fragment, &meta, "preserve_fragment")?;
            } else {
                return Err(meta.error("unsupported inertia container attribute"));
            }
            Ok(())
        })?;
    }
    Ok(output)
}

fn set_flag(current: bool, meta: &syn::meta::ParseNestedMeta<'_>, name: &str) -> syn::Result<bool> {
    if current {
        Err(meta.error(format!("duplicate inertia {name}")))
    } else {
        Ok(true)
    }
}

#[derive(Default)]
pub(crate) struct FieldAttributes {
    pub rename: Option<LitStr>,
    pub skip: bool,
}

pub(crate) fn field(attributes: &[Attribute]) -> syn::Result<FieldAttributes> {
    let mut output = FieldAttributes::default();
    for attribute in attributes
        .iter()
        .filter(|attribute| attribute.path().is_ident("inertia"))
    {
        attribute.parse_nested_meta(|meta| {
            if meta.path.is_ident("rename") {
                if output.rename.is_some() {
                    return Err(meta.error("duplicate inertia field rename"));
                }
                output.rename = Some(meta.value()?.parse()?);
            } else if meta.path.is_ident("skip") {
                if output.skip {
                    return Err(meta.error("duplicate inertia field skip"));
                }
                output.skip = true;
            } else {
                return Err(meta.error("unsupported inertia field attribute"));
            }
            Ok(())
        })?;
    }
    if output.skip && output.rename.is_some() {
        return Err(error(
            attributes
                .first()
                .map_or_else(proc_macro2::Span::call_site, Spanned::span),
            "inertia skip and rename cannot be combined",
        ));
    }
    Ok(output)
}
