use super::model::{OpaqueBorrowConfig, OpaqueDeclArgs, OpaqueDeclConfig};
use darling::FromMeta;
use darling::ast::NestedMeta;
use syn::{FieldsNamed, Generics, Lifetime, Meta, Token, punctuated::Punctuated};

pub(super) fn validate_opaque_generics(generics: &Generics) -> syn::Result<Lifetime> {
    if !generics.type_params().collect::<Vec<_>>().is_empty() {
        return Err(syn::Error::new_spanned(
            generics,
            "#[qt_entity(opaque, ...)] does not support type parameters",
        ));
    }

    let mut lifetime_params = generics.lifetimes();
    let Some(lifetime) = lifetime_params.next().map(|param| param.lifetime.clone()) else {
        return Err(syn::Error::new_spanned(
            generics,
            "#[qt_entity(opaque, ...)] requires exactly one lifetime parameter",
        ));
    };
    if lifetime_params.next().is_some() {
        return Err(syn::Error::new_spanned(
            generics,
            "#[qt_entity(opaque, ...)] supports exactly one lifetime parameter",
        ));
    }

    Ok(lifetime)
}

pub(super) fn reject_qt_field_attrs(fields: &FieldsNamed) -> syn::Result<()> {
    if let Some(field) = fields
        .named
        .iter()
        .find(|field| field.attrs.iter().any(|attr| attr.path().is_ident("qt")))
    {
        return Err(syn::Error::new_spanned(
            field,
            "#[qt_entity(opaque, ...)] does not support field-level #[qt(...)]",
        ));
    }

    Ok(())
}

pub(super) fn parse_qt_opaque_config(
    meta_items: Punctuated<syn::Meta, Token![,]>,
) -> syn::Result<OpaqueDeclConfig> {
    let meta_items = meta_items.into_iter().collect::<Vec<_>>();
    for meta in &meta_items {
        match meta {
            Meta::NameValue(meta)
                if meta.path.is_ident("class") || meta.path.is_ident("include") =>
            {
                return Err(syn::Error::new_spanned(
                    meta,
                    "opaque host class/include moved under host(class = ..., include = ...)",
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
    let args = OpaqueDeclArgs::from_list(&nested)
        .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
    let borrow = args
        .borrow
        .ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[qt_entity(opaque, ...)] requires borrow = \"ref\"|\"mut\"",
            )
        })?
        .value();

    Ok(OpaqueDeclConfig {
        host: args.host.ok_or_else(|| {
            syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[qt_entity(opaque, ...)] requires host(class = ..., include = ...)",
            )
        })?,
        borrow: match borrow.as_str() {
            "ref" => OpaqueBorrowConfig::Ref,
            "mut" => OpaqueBorrowConfig::Mut,
            _ => {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "borrow = ... expects \"ref\" or \"mut\"",
                ));
            }
        },
    })
}

#[cfg(test)]
mod tests {
    use super::{parse_qt_opaque_config, reject_qt_field_attrs, validate_opaque_generics};
    use crate::opaque::model::OpaqueBorrowConfig;
    use syn::parse_quote;

    #[test]
    fn parse_opaque_config_accepts_ref_borrow() {
        let config = parse_qt_opaque_config(parse_quote!(
            borrow = "ref",
            host(class = "QObject", include = "<QObject>")
        ))
        .expect("opaque config parses");

        assert!(matches!(config.borrow, OpaqueBorrowConfig::Ref));
        assert_eq!(config.host.class.value(), "QObject");
        assert_eq!(config.host.include.value(), "<QObject>");
    }

    #[test]
    fn parse_opaque_config_rejects_unknown_borrow() {
        let error = parse_qt_opaque_config(parse_quote!(
            borrow = "owned",
            host(class = "QObject", include = "<QObject>")
        ))
        .err()
        .expect("unknown borrow must fail");

        assert!(
            error
                .to_string()
                .contains("borrow = ... expects \"ref\" or \"mut\"")
        );
    }

    #[test]
    fn opaque_rejects_field_level_qt_attrs() {
        let fields: syn::FieldsNamed = parse_quote!({
            #[qt(prop = text)]
            text: String,
            marker: core::marker::PhantomData<&'a ()>,
        });

        let error = reject_qt_field_attrs(&fields)
            .err()
            .expect("field-level qt attrs must fail for opaque");

        assert!(
            error
                .to_string()
                .contains("does not support field-level #[qt(...)]")
        );
    }

    #[test]
    fn opaque_requires_one_lifetime_parameter() {
        let error = validate_opaque_generics(&parse_quote!())
            .err()
            .expect("missing lifetime must fail");

        assert!(
            error
                .to_string()
                .contains("requires exactly one lifetime parameter")
        );
    }

    #[test]
    fn opaque_rejects_type_parameters() {
        let error = validate_opaque_generics(&parse_quote!(<'a, T>))
            .err()
            .expect("type params must fail");

        assert!(
            error
                .to_string()
                .contains("does not support type parameters")
        );
    }
}
