use super::model::{MethodConfig, MethodLoweringConfig};
use darling::FromMeta;
use darling::ast::NestedMeta;
use syn::spanned::Spanned;
use syn::{Attribute, Meta, Token, parse::Parser, punctuated::Punctuated};

pub(super) fn take_item_level_host_attr(
    attrs: &mut Vec<Attribute>,
) -> syn::Result<Option<proc_macro2::TokenStream>> {
    let mut retained = Vec::with_capacity(attrs.len());
    let mut host_attr = None;

    for attr in std::mem::take(attrs) {
        if !attr.path().is_ident("qt") {
            retained.push(attr);
            continue;
        }

        if host_attr.is_some() {
            return Err(syn::Error::new_spanned(
                &attr,
                "#[qt_methods] only supports one item-level #[qt(host)]",
            ));
        }

        let parsed = Punctuated::<Meta, Token![,]>::parse_terminated
            .parse2(attr.meta.require_list()?.tokens.clone())?;
        let parsed_items = parsed.iter().collect::<Vec<_>>();
        let Some((head, tail)) = parsed_items.split_first() else {
            return Err(syn::Error::new_spanned(
                &attr,
                "#[qt_methods] item-level #[qt(...)] requires host or host(...)",
            ));
        };

        let forwarded = match head {
            Meta::Path(path) if path.is_ident("host") => {
                if !tail.is_empty() {
                    return Err(syn::Error::new_spanned(
                        tail[0],
                        "#[qt(host)] does not accept extra top-level options",
                    ));
                }
                proc_macro2::TokenStream::new()
            }
            Meta::List(list) if list.path.is_ident("host") => {
                if !tail.is_empty() {
                    return Err(syn::Error::new_spanned(
                        tail[0],
                        "#[qt(host(...))] does not accept extra top-level options",
                    ));
                }
                list.tokens.clone()
            }
            other => {
                return Err(syn::Error::new_spanned(
                    other,
                    "#[qt_methods] item-level #[qt(...)] only supports host or host(...)",
                ));
            }
        };

        host_attr = Some(forwarded);
    }

    *attrs = retained;
    Ok(host_attr)
}

pub(super) fn parse_method_config(attrs: &[Attribute]) -> syn::Result<MethodConfig> {
    let mut includes = Vec::new();
    let mut nested = Vec::new();

    for attr in attrs {
        if !attr.path().is_ident("qt") {
            continue;
        }
        let list = attr.meta.require_list()?;
        for item in NestedMeta::parse_meta_list(list.tokens.clone())? {
            match &item {
                NestedMeta::Meta(Meta::NameValue(meta)) if meta.path.is_ident("include") => {
                    includes.push(crate::common::parse_name_value_string(&meta.value)?);
                }
                NestedMeta::Meta(Meta::NameValue(meta)) if meta.path.is_ident("bind") => {
                    return Err(syn::Error::new(
                        meta.path.span(),
                        "#[qt_methods] no longer supports bind = ...; use #[qt(host)] or #[qt(notify = ...)]",
                    ));
                }
                NestedMeta::Meta(Meta::Path(path)) if path.is_ident("raw") => {
                    return Err(syn::Error::new_spanned(
                        path,
                        "plain method is already raw; remove #[qt(raw)]",
                    ));
                }
                NestedMeta::Meta(Meta::NameValue(meta)) if meta.path.is_ident("raw") => {
                    return Err(syn::Error::new_spanned(
                        meta,
                        "plain method is already raw; remove #[qt(raw)]",
                    ));
                }
                _ => nested.push(item),
            }
        }
    }

    let args = MethodAttrArgs::from_list(&nested)
        .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
    let lowering = if let Some(host) = args.host {
        MethodLoweringConfig::Host {
            host_name_override: host.0,
        }
    } else {
        MethodLoweringConfig::Plain
    };

    if !includes.is_empty() && !matches!(lowering, MethodLoweringConfig::Host { .. }) {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[qt(include = ...)] requires #[qt(host)] on the same method",
        ));
    }

    Ok(MethodConfig {
        lowering,
        extra_includes: includes,
    })
}

#[derive(Default, FromMeta)]
struct MethodAttrArgs {
    #[darling(default)]
    host: Option<HostNameOverrideArg>,
}

struct HostNameOverrideArg(Option<String>);

impl FromMeta for HostNameOverrideArg {
    fn from_word() -> darling::Result<Self> {
        Ok(Self(None))
    }

    fn from_expr(expr: &syn::Expr) -> darling::Result<Self> {
        crate::common::parse_name_value_string(expr)
            .map(|value| Self(Some(value.value())))
            .map_err(|error| darling::Error::custom(error.to_string()).with_span(expr))
    }

    fn from_list(items: &[NestedMeta]) -> darling::Result<Self> {
        let [NestedMeta::Lit(syn::Lit::Str(value))] = items else {
            return Err(darling::Error::custom(
                "#[qt(host(...))] currently accepts no args, = \"...\", or (\"...\")",
            ));
        };
        Ok(Self(Some(value.value())))
    }
}
