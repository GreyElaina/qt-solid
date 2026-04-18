use super::model::{WidgetDeclArgs, WidgetDeclConfig, WidgetFieldDefault};
use crate::common::{ident_to_lower_camel, single_ident};
use darling::FromMeta;
use darling::ast::NestedMeta;
use quote::quote;
use syn::{Expr, Field, Ident, LitStr, Token, punctuated::Punctuated};

pub(super) fn parse_widget_field_default(field: &Field) -> syn::Result<WidgetFieldDefault> {
    let ident = field.ident.clone().expect("named field");
    let mut default = None;

    for attr in &field.attrs {
        if !attr.path().is_ident("qt") {
            continue;
        }
        let list = attr.meta.require_list()?;
        for item in NestedMeta::parse_meta_list(list.tokens.clone())? {
            match item {
                NestedMeta::Meta(syn::Meta::Path(path)) if path.is_ident("default") => {
                    default = Some(quote!(core::default::Default::default()));
                }
                NestedMeta::Meta(syn::Meta::NameValue(meta)) if meta.path.is_ident("default") => {
                    let expr: Expr = meta.value;
                    default = Some(match expr {
                        Expr::Path(path) => quote!(#path()),
                        other => quote!(#other),
                    });
                }
                NestedMeta::Meta(syn::Meta::Path(path))
                    if path.is_ident("prop")
                        || path.is_ident("const")
                        || path.is_ident("command")
                        || path.is_ident("export") => {}
                NestedMeta::Meta(syn::Meta::NameValue(meta))
                    if meta.path.is_ident("prop")
                        || meta.path.is_ident("const")
                        || meta.path.is_ident("command")
                        || meta.path.is_ident("export") => {}
                NestedMeta::Meta(syn::Meta::List(meta))
                    if meta.path.is_ident("prop")
                        || meta.path.is_ident("const")
                        || meta.path.is_ident("command")
                        || meta.path.is_ident("export") => {}
                NestedMeta::Meta(syn::Meta::Path(path)) if path.is_ident("meta") => {
                    return Err(syn::Error::new_spanned(
                        path,
                        "#[qt(meta(...))] removed; field prop lowering derives from #[qt(prop = ...)]",
                    ));
                }
                NestedMeta::Meta(syn::Meta::NameValue(meta)) if meta.path.is_ident("meta") => {
                    return Err(syn::Error::new_spanned(
                        meta,
                        "#[qt(meta(...))] removed; field prop lowering derives from #[qt(prop = ...)]",
                    ));
                }
                NestedMeta::Meta(syn::Meta::List(meta)) if meta.path.is_ident("meta") => {
                    return Err(syn::Error::new_spanned(
                        meta,
                        "#[qt(meta(...))] removed; field prop lowering derives from #[qt(prop = ...)]",
                    ));
                }
                NestedMeta::Meta(meta) => {
                    return Err(syn::Error::new_spanned(
                        meta,
                        "unsupported #[qt(...)] widget field option",
                    ));
                }
                NestedMeta::Lit(lit) => {
                    return Err(syn::Error::new_spanned(
                        lit,
                        "unsupported #[qt(...)] widget field option",
                    ));
                }
            }
        }
    }

    Ok(WidgetFieldDefault { ident, default })
}

pub(super) fn parse_widget_decl_config(
    meta_items: Punctuated<syn::Meta, Token![,]>,
) -> syn::Result<WidgetDeclConfig> {
    let meta_items = meta_items.into_iter().collect::<Vec<_>>();

    for meta in &meta_items {
        match meta {
            syn::Meta::NameValue(meta) if meta.path.is_ident("kind") => {
                return Err(syn::Error::new_spanned(
                    meta,
                    "kind = ... removed; internal widget kind derives automatically",
                ));
            }
            syn::Meta::NameValue(meta) if meta.path.is_ident("props") => {
                return Err(syn::Error::new_spanned(
                    meta,
                    "props = ... removed; collect props from #[qt(host)] and #[qt_methods]",
                ));
            }
            syn::Meta::NameValue(meta) if meta.path.is_ident("methods") => {
                return Err(syn::Error::new_spanned(
                    meta,
                    "methods = ... removed; #[qt_methods] now registers widget methods automatically",
                ));
            }
            syn::Meta::List(meta) if meta.path.is_ident("use") => {
                return Err(syn::Error::new_spanned(
                    meta,
                    "use(...) removed; attach capability traits with #[qt(host)] impl Trait for Widget {}",
                ));
            }
            syn::Meta::List(meta)
                if matches!(
                    meta.path
                        .get_ident()
                        .map(|ident| ident.to_string())
                        .as_deref(),
                    Some("window" | "focus" | "layout" | "font" | "range" | "selection")
                ) =>
            {
                return Err(syn::Error::new_spanned(
                    meta,
                    "widget capability metadata removed; express host facts from #[qt(host)] traits instead",
                ));
            }
            _ => {}
        }
    }

    let nested = meta_items
        .iter()
        .cloned()
        .map(NestedMeta::Meta)
        .collect::<Vec<_>>();
    let args = WidgetDeclArgs::from_list(&nested)
        .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;

    Ok(WidgetDeclConfig {
        export: args.export,
        children: args
            .children
            .as_ref()
            .map(|path| single_ident(path, "children = ... expects a single identifier"))
            .transpose()?
            .unwrap_or_else(|| syn::parse_quote!(None)),
        host: args.host,
    })
}

pub(super) fn resolve_widget_export_name(struct_name: &Ident, config: &WidgetDeclConfig) -> LitStr {
    if let Some(export) = &config.export {
        return export.clone();
    }

    LitStr::new(
        &ident_to_lower_camel(&struct_name.to_string()),
        struct_name.span(),
    )
}

#[cfg(test)]
mod tests {
    use super::{parse_widget_field_default, resolve_widget_export_name};
    use crate::widget::model::WidgetDeclConfig;
    use quote::format_ident;
    use syn::parse_quote;

    fn test_config(export: Option<&str>) -> WidgetDeclConfig {
        WidgetDeclConfig {
            export: export.map(|value| syn::LitStr::new(value, proc_macro2::Span::call_site())),
            children: syn::parse_quote!(None),
            host: None,
        }
    }

    #[test]
    fn export_name_prefers_explicit_export() {
        let config = test_config(Some("window"));
        let name = resolve_widget_export_name(&format_ident!("WindowWidget"), &config);

        assert_eq!(name.value(), "window");
    }

    #[test]
    fn export_name_defaults_to_struct_ident() {
        let config = test_config(None);
        let name = resolve_widget_export_name(&format_ident!("WindowWidget"), &config);

        assert_eq!(name.value(), "windowWidget");
    }

    #[test]
    fn widget_field_default_parser_allows_prop_attrs() {
        let input: syn::ItemStruct = parse_quote! {
            struct Demo {
                #[qt(prop = text)]
                text: String,
            }
        };
        let field = input.fields.iter().next().expect("field");

        let parsed = parse_widget_field_default(field).expect("field parses");
        assert!(parsed.default.is_none());
    }

    #[test]
    fn widget_field_default_parser_keeps_default_expr() {
        let input: syn::ItemStruct = parse_quote! {
            struct Demo {
                #[qt(default = 7)]
                frame_seq: u64,
            }
        };
        let field = input.fields.iter().next().expect("field");

        let parsed = parse_widget_field_default(field).expect("field parses");
        assert!(parsed.default.is_some());
    }
}
