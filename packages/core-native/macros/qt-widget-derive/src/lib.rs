use proc_macro::TokenStream;
use syn::parse::Parser;
use syn::parse_macro_input;

mod binding;
mod common;
mod methods;
mod opaque;
mod prop_methods;
mod props;
mod surface;
mod widget;

#[proc_macro_attribute]
pub fn qt_entity(attr: TokenStream, item: TokenStream) -> TokenStream {
    match surface::expand_qt_entity_attr(
        proc_macro2::TokenStream::from(attr),
        parse_macro_input!(item as syn::Item),
    ) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_attribute]
pub fn qt_methods(_attr: TokenStream, item: TokenStream) -> TokenStream {
    match methods::expand_qt_methods_attr(parse_macro_input!(item as syn::Item)) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

#[proc_macro_derive(Qt, attributes(qt))]
pub fn derive_qt(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::DeriveInput);
    match expand_qt_derive(input) {
        Ok(tokens) => tokens.into(),
        Err(error) => error.to_compile_error().into(),
    }
}

fn expand_qt_derive(input: syn::DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    reject_entity_attr_on_derive(&input)?;
    let prop_tokens = if props::should_expand_qt_prop_tree(&input) {
        props::expand_qt_prop_tree(input.clone())?
    } else {
        proc_macro2::TokenStream::new()
    };

    Ok(quote::quote! {
        #prop_tokens
    })
}

fn reject_entity_attr_on_derive(input: &syn::DeriveInput) -> syn::Result<()> {
    let struct_attr = input
        .attrs
        .iter()
        .find(|attr| attr.path().is_ident("qt"))
        .map(|attr| {
            syn::punctuated::Punctuated::<syn::Meta, syn::Token![,]>::parse_terminated
                .parse2(attr.meta.require_list()?.tokens.clone())
        })
        .transpose()?;
    let Some(meta_items) = struct_attr else {
        return Ok(());
    };

    let parsed_items = meta_items.iter().cloned().collect::<Vec<_>>();
    let Some((head, _tail)) = parsed_items.split_first() else {
        return Ok(());
    };

    match head {
        syn::Meta::Path(path) if path.is_ident("widget") || path.is_ident("opaque") => {
            Err(syn::Error::new_spanned(
                path,
                "entity-level #[qt(...)] was removed; use #[qt_entity(...)]",
            ))
        }
        syn::Meta::List(list) if list.path.is_ident("widget") || list.path.is_ident("opaque") => {
            Err(syn::Error::new_spanned(
                list,
                "entity-level #[qt(...)] was removed; use #[qt_entity(...)]",
            ))
        }
        other => Err(syn::Error::new_spanned(
            other,
            "top-level #[qt(...)] is not supported on #[derive(Qt)] structs; use #[qt_entity(...)]",
        )),
    }
}

#[cfg(test)]
mod tests {
    use super::reject_entity_attr_on_derive;
    use syn::parse_quote;

    #[test]
    fn derive_rejects_widget_entity_attr() {
        let input: syn::DeriveInput = parse_quote! {
            #[qt(widget, export = "demo")]
            struct Demo;
        };

        let error = reject_entity_attr_on_derive(&input)
            .err()
            .expect("legacy derive entity attr must be rejected");

        assert!(
            error
                .to_string()
                .contains("entity-level #[qt(...)] was removed; use #[qt_entity(...)]")
        );
    }
}
