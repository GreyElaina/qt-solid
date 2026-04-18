use proc_macro_crate::{FoundCrate, crate_name};
use quote::quote;
use syn::{Expr, Ident, Lit, LitStr, Path, Type};

#[derive(Clone)]
pub(crate) struct FlatPropName {
    pub rust_name: String,
    pub js_name: String,
}

pub(crate) fn widget_core_root() -> proc_macro2::TokenStream {
    match crate_name("qt-solid-widget-core") {
        Ok(FoundCrate::Itself) => quote!(crate),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, proc_macro2::Span::call_site());
            quote!(::#ident)
        }
        Err(error) => panic!("failed to resolve qt-solid-widget-core dependency: {error}"),
    }
}

pub(crate) fn widget_core_decl_path() -> proc_macro2::TokenStream {
    let root = widget_core_root();
    quote!(#root::decl)
}

pub(crate) fn widget_core_schema_path() -> proc_macro2::TokenStream {
    let root = widget_core_root();
    quote!(#root::schema)
}

pub(crate) fn widget_core_runtime_path() -> proc_macro2::TokenStream {
    let root = widget_core_root();
    quote!(#root::runtime)
}

pub(crate) fn widget_core_codegen_path() -> proc_macro2::TokenStream {
    let root = widget_core_root();
    quote!(#root::codegen)
}

pub(crate) fn parse_name_value_string(expr: &Expr) -> syn::Result<LitStr> {
    let Expr::Lit(expr_lit) = expr else {
        return Err(syn::Error::new_spanned(expr, "expected string literal"));
    };
    let Lit::Str(value) = &expr_lit.lit else {
        return Err(syn::Error::new_spanned(expr, "expected string literal"));
    };
    Ok(value.clone())
}

pub(crate) fn option_inner_type(ty: &Type) -> Option<&Type> {
    let Type::Path(type_path) = ty else {
        return None;
    };

    let segment = type_path.path.segments.last()?;
    if segment.ident != "Option" {
        return None;
    }

    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return None;
    };

    let syn::GenericArgument::Type(inner) = arguments.args.first()? else {
        return None;
    };

    Some(inner)
}

pub(crate) fn parse_flat_prop_name_expr(expr: Expr) -> syn::Result<FlatPropName> {
    let Expr::Path(path) = expr else {
        return Err(syn::Error::new_spanned(
            expr,
            "prop = ... expects prop name",
        ));
    };
    if path.path.segments.len() != 1 {
        return Err(syn::Error::new_spanned(
            path,
            "prop = ... expects flat prop name",
        ));
    }

    let ident = &path.path.segments[0].ident;
    Ok(FlatPropName {
        rust_name: ident.to_string(),
        js_name: snake_to_lower_camel(&ident.to_string()),
    })
}

pub(crate) fn snake_to_lower_camel(value: &str) -> String {
    let mut result = String::new();
    let mut upper_next = false;

    for character in value.chars() {
        if character == '_' {
            upper_next = true;
            continue;
        }

        if upper_next {
            result.extend(character.to_uppercase());
            upper_next = false;
        } else {
            result.push(character);
        }
    }

    result
}

pub(crate) fn ident_to_lower_camel(value: &str) -> String {
    let mut result = String::new();

    for (index, character) in value.chars().enumerate() {
        if character.is_uppercase() {
            if index == 0 {
                result.extend(character.to_lowercase());
            } else {
                result.push(character);
            }
        } else {
            result.push(character);
        }
    }

    result
}

pub(crate) fn sanitize_cpp_symbol_segment(value: &str) -> String {
    let mut out = String::new();

    for ch in value.chars() {
        if ch.is_ascii_alphanumeric() || ch == '_' {
            out.push(ch.to_ascii_lowercase());
        } else {
            out.push('_');
        }
    }

    out
}

pub(crate) fn single_ident(path: &Path, error: &str) -> syn::Result<Ident> {
    if path.segments.len() != 1 {
        return Err(syn::Error::new_spanned(path, error));
    }
    Ok(path.segments[0].ident.clone())
}
