use crate::{opaque, widget};
use quote::quote;
use syn::{Item, Meta, Token, parse::Parser, punctuated::Punctuated};

pub(crate) fn expand_qt_entity_attr(
    attr: proc_macro2::TokenStream,
    item: Item,
) -> syn::Result<proc_macro2::TokenStream> {
    match item {
        Item::Struct(input) => expand_qt_struct_attr(attr, input),
        other => Err(syn::Error::new_spanned(
            other,
            "#[qt_entity(...)] only supports structs",
        )),
    }
}

fn expand_qt_struct_attr(
    attr: proc_macro2::TokenStream,
    input: syn::ItemStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    let parsed = parse_meta_items(attr)?.into_iter().collect::<Vec<_>>();
    let Some((head, tail)) = parsed.split_first() else {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[qt_entity(...)] requires widget or opaque mode",
        ));
    };

    match head {
        Meta::Path(path) if path.is_ident("widget") => {
            widget::expand_qt_widget_attr(meta_items_from_slice(tail)?, input)
        }
        Meta::Path(path) if path.is_ident("opaque") => {
            opaque::expand_qt_opaque_attr(meta_items_from_slice(tail)?, input)
        }
        Meta::List(list) if list.path.is_ident("widget") => {
            if !tail.is_empty() {
                return Err(syn::Error::new_spanned(
                    &tail[0],
                    "#[qt_entity(widget(...))] does not accept extra top-level options",
                ));
            }

            widget::expand_qt_widget_attr(parse_meta_items(list.tokens.clone())?, input)
        }
        Meta::List(list) if list.path.is_ident("opaque") => Err(syn::Error::new_spanned(
            list,
            "#[qt_entity(opaque(...))] was removed; use #[qt_entity(opaque, borrow = ..., host(class = ..., include = ...))]",
        )),
        other => Err(syn::Error::new_spanned(
            other,
            "#[qt_entity(...)] struct mode expects widget or opaque",
        )),
    }
}

pub(crate) fn parse_meta_items(
    attr: proc_macro2::TokenStream,
) -> syn::Result<Punctuated<Meta, Token![,]>> {
    Punctuated::<Meta, Token![,]>::parse_terminated.parse2(attr)
}

fn meta_items_from_slice(items: &[Meta]) -> syn::Result<Punctuated<Meta, Token![,]>> {
    let tokens = quote!(#(#items),*);
    parse_meta_items(tokens)
}

#[cfg(test)]
mod tests {
    use super::{expand_qt_entity_attr, meta_items_from_slice, parse_meta_items};

    #[test]
    fn parses_widget_head_with_tail() {
        let parsed = parse_meta_items(syn::parse_quote!(
            widget,
            export = "window",
            children = Nodes
        ))
        .expect("qt attr parses");
        let items = parsed.into_iter().collect::<Vec<_>>();

        assert!(matches!(&items[0], syn::Meta::Path(path) if path.is_ident("widget")));
        assert!(matches!(&items[1], syn::Meta::NameValue(meta) if meta.path.is_ident("export")));
        assert!(matches!(&items[2], syn::Meta::NameValue(meta) if meta.path.is_ident("children")));
    }

    #[test]
    fn rebuilds_tail_meta_items() {
        let parsed = parse_meta_items(syn::parse_quote!(
            widget,
            export = "window",
            children = Nodes
        ))
        .expect("qt attr parses");
        let items = parsed.into_iter().collect::<Vec<_>>();
        let rebuilt = meta_items_from_slice(&items[1..]).expect("tail rebuilds");
        let rebuilt_items = rebuilt.into_iter().collect::<Vec<_>>();

        assert_eq!(rebuilt_items.len(), 2);
        assert!(
            matches!(&rebuilt_items[0], syn::Meta::NameValue(meta) if meta.path.is_ident("export"))
        );
        assert!(
            matches!(&rebuilt_items[1], syn::Meta::NameValue(meta) if meta.path.is_ident("children"))
        );
    }

    #[test]
    fn rejects_legacy_opaque_head_list() {
        let item: syn::Item = syn::parse_quote! {
            struct Painter<'a> {}
        };

        let error = expand_qt_entity_attr(
            syn::parse_quote!(opaque(class = "QPainter", include = "<QtGui/QPainter>")),
            item,
        )
        .err()
        .expect("legacy opaque(...) head must be rejected");

        assert!(
            error
                .to_string()
                .contains("#[qt_entity(opaque(...))] was removed")
        );
    }
}
