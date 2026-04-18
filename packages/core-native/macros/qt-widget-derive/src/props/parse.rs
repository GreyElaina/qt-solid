use super::model::{FieldAttrArgs, FieldConfig, FieldPropConfig, PropBehaviorConfig};
use darling::FromMeta;
use darling::ast::NestedMeta;
use syn::{Field, Meta};

pub(crate) fn struct_declares_widget_props(fields: &[Field]) -> syn::Result<bool> {
    Ok(fields
        .iter()
        .map(parse_field_config)
        .collect::<syn::Result<Vec<_>>>()?
        .iter()
        .any(|config| config.prop.is_some()))
}

pub(super) fn collect_prop_fields(
    fields: &syn::punctuated::Punctuated<Field, syn::token::Comma>,
) -> syn::Result<Vec<Field>> {
    let mut prop_fields = Vec::new();

    for field in fields {
        let config = parse_field_config(field)?;
        if config.prop.is_some() {
            prop_fields.push(field.clone());
        }
    }

    Ok(prop_fields)
}

pub(super) fn parse_field_config(field: &Field) -> syn::Result<FieldConfig> {
    let nested = collect_qt_nested_meta(&field.attrs)?;
    for item in &nested {
        let NestedMeta::Meta(meta) = item else {
            continue;
        };
        match meta {
            Meta::Path(path) if path.is_ident("props") => {
                return Err(syn::Error::new_spanned(
                    path,
                    "#[qt(props)] removed; attach capability traits with #[qt(host)] or declare flat props directly",
                ));
            }
            Meta::Path(path) if path.is_ident("group") => {
                return Err(syn::Error::new_spanned(
                    path,
                    "#[qt(group)] removed; grouped props are no longer supported",
                ));
            }
            Meta::List(list) if list.path.is_ident("meta") => {
                return Err(syn::Error::new_spanned(
                    list,
                    "#[qt(meta(...))] removed; field prop lowering derives from #[qt(prop = ...)]",
                ));
            }
            Meta::NameValue(meta) if meta.path.is_ident("meta") => {
                return Err(syn::Error::new_spanned(
                    meta,
                    "#[qt(meta(...))] removed; field prop lowering derives from #[qt(prop = ...)]",
                ));
            }
            _ => {}
        }
    }

    let args = FieldAttrArgs::from_list(&nested)
        .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
    if args.is_const && args.command {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[qt(const)] and #[qt(command)] are mutually exclusive",
        ));
    }

    Ok(FieldConfig {
        prop: args.prop.map(|prop| FieldPropConfig {
            rust_name: prop.0.rust_name,
            js_name: prop.0.js_name,
        }),
        default: args.default.map(|default| default.0),
        behavior: if args.is_const {
            PropBehaviorConfig::Const
        } else if args.command {
            PropBehaviorConfig::Command
        } else {
            PropBehaviorConfig::State
        },
        exported: args.export,
    })
}

fn collect_qt_nested_meta(attrs: &[syn::Attribute]) -> syn::Result<Vec<NestedMeta>> {
    let mut nested = Vec::new();

    for attr in attrs {
        if !attr.path().is_ident("qt") {
            continue;
        }
        let list = attr.meta.require_list()?;
        nested.extend(NestedMeta::parse_meta_list(list.tokens.clone())?);
    }

    Ok(nested)
}

#[cfg(test)]
mod tests {
    use super::{parse_field_config, struct_declares_widget_props};
    use syn::parse_quote;

    #[test]
    fn parse_flat_prop_name_without_extra_lowering() {
        let field: syn::Field = parse_quote! {
            #[qt(prop = text)]
            text: Option<String>
        };

        let config = parse_field_config(&field).expect("field config");
        let prop = config.prop.expect("prop");
        assert_eq!(prop.rust_name, "text");
        assert_eq!(prop.js_name, "text");
    }

    #[test]
    fn field_meta_attr_is_removed() {
        let field: syn::Field = parse_quote! {
            #[qt(prop = text, meta("text"))]
            text: Option<String>
        };

        let error = parse_field_config(&field)
            .err()
            .expect("meta attr must be rejected");

        assert!(
            error
                .to_string()
                .contains("field prop lowering derives from #[qt(prop = ...)]")
        );
    }

    #[test]
    fn default_only_fields_do_not_mark_struct_as_props() {
        let fields = vec![parse_quote! {
            #[qt(default)]
            frame_seq: u64
        }];

        assert!(
            !struct_declares_widget_props(&fields).expect("prop detection"),
            "default-only fields must not become widget Props",
        );
    }
}
