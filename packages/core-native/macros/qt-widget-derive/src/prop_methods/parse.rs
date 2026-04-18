use super::model::{
    ConstructorParamSpec, ManualPropKind, ManualPropMethod, QtMethodImplKind, QtPropMethodConfig,
};
use crate::common::{option_inner_type, parse_flat_prop_name_expr, snake_to_lower_camel};
use darling::FromMeta;
use darling::ast::NestedMeta;
use syn::{
    Expr, FnArg, Ident, ItemImpl, Meta, Pat, ReturnType, Token, Type, parse::Parser,
    punctuated::Punctuated,
};

pub(crate) fn classify_qt_method_impl(input: &ItemImpl) -> syn::Result<QtMethodImplKind> {
    let mut saw_prop = false;
    let mut saw_other_qt = false;

    for item in &input.items {
        let syn::ImplItem::Fn(method) = item else {
            continue;
        };

        let method_kind = classify_qt_prop_method_attrs(&method.attrs)?;
        saw_prop |= method_kind.has_prop;
        saw_other_qt |= method_kind.has_other_qt;
    }

    Ok(match (saw_prop, saw_other_qt) {
        (false, _) => QtMethodImplKind::None,
        (true, false) => QtMethodImplKind::Pure,
        (true, true) => QtMethodImplKind::Mixed,
    })
}

pub(crate) fn method_uses_qt_prop_attrs(attrs: &[syn::Attribute]) -> syn::Result<bool> {
    Ok(classify_qt_prop_method_attrs(attrs)?.has_prop)
}

pub(crate) fn parse_qt_prop_method_config(
    attrs: &[syn::Attribute],
) -> syn::Result<QtPropMethodConfig> {
    let nested = collect_qt_nested_meta(attrs)?;
    let args = QtPropMethodAttrArgs::from_list(&nested)
        .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;

    if args.constructor {
        if args.prop.is_some()
            || args.setter
            || args.getter
            || args.init
            || args.update
            || args.default.is_some()
        {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[qt(constructor)] cannot be combined with prop/getter/setter/init/update/default",
            ));
        }
        return Ok(QtPropMethodConfig::Constructor);
    }

    let Some(prop_name) = args.prop.map(|prop| prop.0) else {
        return Ok(QtPropMethodConfig::Plain);
    };

    if args.setter == args.getter {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "widget prop methods require exactly one of #[qt(setter)] or #[qt(getter)]",
        ));
    }
    if (args.init || args.update) && !args.setter {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[qt(init)] and #[qt(update)] require #[qt(setter)]",
        ));
    }
    if args.init && args.update {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[qt(init)] and #[qt(update)] are mutually exclusive",
        ));
    }

    Ok(QtPropMethodConfig::Prop(ManualPropMethod {
        js_name: prop_name.js_name,
        kind: if args.setter {
            ManualPropKind::Setter
        } else {
            ManualPropKind::Getter
        },
        init: args.init,
        default: args.default.map(|expr| expr.0),
    }))
}

pub(crate) fn parse_constructor_params(
    sig: &mut syn::Signature,
) -> syn::Result<Vec<ConstructorParamSpec>> {
    let mut params = Vec::new();

    for arg in sig.inputs.iter_mut() {
        let FnArg::Typed(arg) = arg else {
            continue;
        };
        let Pat::Ident(pat_ident) = arg.pat.as_ref() else {
            return Err(syn::Error::new_spanned(
                &arg.pat,
                "#[qt(constructor)] parameters require simple identifiers",
            ));
        };

        let mut retained_attrs = Vec::new();

        for attr in std::mem::take(&mut arg.attrs) {
            if !attr.path().is_ident("qt") {
                retained_attrs.push(attr);
                continue;
            }
            retained_attrs.push(attr);
        }

        let args = constructor_param_attr_args(&retained_attrs)?;
        arg.attrs = retained_attrs
            .into_iter()
            .filter(|attr| !attr.path().is_ident("qt"))
            .collect();
        let js_name = match args.prop {
            Some(ConstructorPropArg::Implicit) => {
                snake_to_lower_camel(&pat_ident.ident.to_string())
            }
            Some(ConstructorPropArg::Named(js_name)) => js_name,
            None => {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[qt(constructor)] parameters require #[qt(prop)]",
                ));
            }
        };
        let default = args.default.map(|default| default.0);

        let arg_ty = (*arg.ty).clone();
        let value_ty = option_inner_type(&arg_ty).unwrap_or(&arg_ty).clone();
        params.push(ConstructorParamSpec {
            arg_ident: pat_ident.ident.clone(),
            js_name,
            arg_ty,
            value_ty,
            default,
            optional: option_inner_type(&arg.ty).is_some(),
        });
    }

    Ok(params)
}

pub(crate) fn render_constructor_param_expr(
    param: &ConstructorParamSpec,
    runtime: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let key = syn::LitStr::new(&param.js_name, proc_macro2::Span::call_site());
    let value_ty = &param.value_ty;
    let arg_ty = &param.arg_ty;

    if param.optional {
        if let Some(default) = &param.default {
            return Ok(quote::quote! {
                match #runtime::parse_widget_create_prop::<#value_ty>(create_props, #key)? {
                    Some(value) => core::option::Option::Some(value),
                    None => #default,
                }
            });
        }

        return Ok(quote::quote! {
            #runtime::parse_widget_create_prop::<#value_ty>(create_props, #key)?
        });
    }

    if let Some(default) = &param.default {
        return Ok(quote::quote! {
            match #runtime::parse_widget_create_prop::<#arg_ty>(create_props, #key)? {
                Some(value) => value,
                None => #default,
            }
        });
    }

    let arg_ident = &param.arg_ident;
    Ok(quote::quote! {
        #runtime::parse_widget_create_prop::<#arg_ty>(create_props, #key)?
            .ok_or_else(|| #runtime::WidgetError::new(format!(
                "missing constructor prop {} for {}",
                #key,
                stringify!(#arg_ident),
            )))?
    })
}

pub(crate) fn validate_constructor_signature(sig: &syn::Signature) -> syn::Result<()> {
    if sig
        .inputs
        .iter()
        .any(|arg| matches!(arg, FnArg::Receiver(_)))
    {
        return Err(syn::Error::new_spanned(
            &sig.inputs,
            "#[qt(constructor)] methods do not accept self receiver",
        ));
    }
    let return_ty = super::method_return_type(&sig.output)?.ok_or_else(|| {
        syn::Error::new_spanned(
            &sig.output,
            "#[qt(constructor)] methods must return Self or WidgetResult<Self>",
        )
    })?;
    let Type::Path(type_path) = return_ty else {
        return Err(syn::Error::new_spanned(
            return_ty,
            "#[qt(constructor)] methods must return Self or WidgetResult<Self>",
        ));
    };
    let last = type_path.path.segments.last().expect("path segment");
    if last.ident != "Self" {
        return Err(syn::Error::new_spanned(
            return_ty,
            "#[qt(constructor)] methods must return Self or WidgetResult<Self>",
        ));
    }
    Ok(())
}

pub(crate) fn validate_qt_prop_setter_signature(sig: &syn::Signature) -> syn::Result<()> {
    let Some(first) = sig.inputs.first() else {
        return Err(syn::Error::new_spanned(
            sig,
            "widget prop setters require &mut self",
        ));
    };
    let FnArg::Receiver(receiver) = first else {
        return Err(syn::Error::new_spanned(
            first,
            "widget prop setters require &mut self",
        ));
    };
    if receiver.reference.is_none() || receiver.mutability.is_none() {
        return Err(syn::Error::new_spanned(
            receiver,
            "widget prop setters require &mut self",
        ));
    }
    if sig.inputs.len() != 2 {
        return Err(syn::Error::new_spanned(
            &sig.inputs,
            "widget prop setters require exactly one value argument",
        ));
    }
    match &sig.output {
        ReturnType::Default => Ok(()),
        ReturnType::Type(_, ty) if super::is_unit_type(ty) => Ok(()),
        ReturnType::Type(_, ty) if super::unwrap_result_type(ty)?.is_some() => Ok(()),
        _ => Err(syn::Error::new_spanned(
            &sig.output,
            "widget prop setters must return unit or WidgetResult<()>",
        )),
    }
}

pub(crate) fn validate_qt_prop_getter_signature(sig: &syn::Signature) -> syn::Result<()> {
    let Some(first) = sig.inputs.first() else {
        return Err(syn::Error::new_spanned(
            sig,
            "widget prop getters require &self",
        ));
    };
    let FnArg::Receiver(receiver) = first else {
        return Err(syn::Error::new_spanned(
            first,
            "widget prop getters require &self",
        ));
    };
    if receiver.reference.is_none() || receiver.mutability.is_some() {
        return Err(syn::Error::new_spanned(
            receiver,
            "widget prop getters require &self",
        ));
    }
    if sig.inputs.len() != 1 {
        return Err(syn::Error::new_spanned(
            &sig.inputs,
            "widget prop getters do not accept explicit arguments",
        ));
    }
    let return_ty = super::method_return_type(&sig.output)?;
    if return_ty.is_none() || return_ty.is_some_and(super::is_unit_type) {
        return Err(syn::Error::new_spanned(
            &sig.output,
            "widget prop getters must return a value",
        ));
    }
    Ok(())
}

#[derive(Default, FromMeta)]
struct QtPropMethodAttrArgs {
    #[darling(default)]
    constructor: bool,
    #[darling(default)]
    prop: Option<AttrFlatPropName>,
    #[darling(default)]
    setter: bool,
    #[darling(default)]
    getter: bool,
    #[darling(default)]
    init: bool,
    #[darling(default)]
    update: bool,
    #[darling(default)]
    default: Option<ExprArg>,
}

#[derive(Clone)]
struct AttrFlatPropName(crate::common::FlatPropName);

impl FromMeta for AttrFlatPropName {
    fn from_meta(item: &Meta) -> darling::Result<Self> {
        match item {
            Meta::NameValue(meta) => parse_flat_prop_name_expr(meta.value.clone())
                .map(Self)
                .map_err(|error| darling::Error::custom(error.to_string()).with_span(meta)),
            Meta::List(meta) => {
                let path = Punctuated::<Ident, Token![::]>::parse_separated_nonempty
                    .parse2(meta.tokens.clone())
                    .map_err(|error| darling::Error::custom(error.to_string()).with_span(meta))?;
                if path.len() != 1 {
                    return Err(
                        darling::Error::custom("prop(...) expects flat prop name").with_span(meta)
                    );
                }
                Ok(Self(crate::common::FlatPropName {
                    rust_name: path[0].to_string(),
                    js_name: snake_to_lower_camel(&path[0].to_string()),
                }))
            }
            other => Err(darling::Error::custom("prop expects prop name").with_span(other)),
        }
    }
}

struct ExprArg(Expr);

impl FromMeta for ExprArg {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        Ok(Self(expr.clone()))
    }
}

#[derive(Default, FromMeta)]
struct ConstructorParamAttrArgs {
    #[darling(default)]
    prop: Option<ConstructorPropArg>,
    #[darling(default)]
    default: Option<ConstructorDefaultArg>,
}

enum ConstructorPropArg {
    Implicit,
    Named(String),
}

impl FromMeta for ConstructorPropArg {
    fn from_word() -> darling::Result<Self> {
        Ok(Self::Implicit)
    }

    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        parse_flat_prop_name_expr(expr.clone())
            .map(|name| Self::Named(name.js_name))
            .map_err(|error| darling::Error::custom(error.to_string()).with_span(expr))
    }
}

struct ConstructorDefaultArg(Expr);

impl FromMeta for ConstructorDefaultArg {
    fn from_word() -> darling::Result<Self> {
        Ok(Self(syn::parse_quote!(core::default::Default::default())))
    }

    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        Ok(Self(expr.clone()))
    }
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

fn constructor_param_attr_args(attrs: &[syn::Attribute]) -> syn::Result<ConstructorParamAttrArgs> {
    let nested = collect_qt_nested_meta(attrs)?;
    ConstructorParamAttrArgs::from_list(&nested)
        .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))
}

struct QtPropMethodAttrKind {
    has_prop: bool,
    has_other_qt: bool,
}

fn classify_qt_prop_method_attrs(attrs: &[syn::Attribute]) -> syn::Result<QtPropMethodAttrKind> {
    let mut has_prop = false;
    let mut has_other_qt = false;

    for item in collect_qt_nested_meta(attrs)? {
        let NestedMeta::Meta(meta) = item else {
            has_other_qt = true;
            continue;
        };
        let path = meta.path();
        if path.is_ident("constructor")
            || path.is_ident("prop")
            || path.is_ident("setter")
            || path.is_ident("getter")
            || path.is_ident("init")
            || path.is_ident("update")
            || path.is_ident("default")
        {
            has_prop = true;
        } else {
            has_other_qt = true;
        }
    }

    Ok(QtPropMethodAttrKind {
        has_prop,
        has_other_qt,
    })
}
