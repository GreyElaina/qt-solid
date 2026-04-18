use super::model::{
    EchoDirectiveArg, HostBehaviorEventConfig, HostCodegenMethodConfig, HostCodegenMethodKind,
    HostConfig, HostDeclHelperKind, HostMethodAttrArgs, HostMethodAttrParts, HostPropBehavior,
    HostPropSpecConfig, HostTraitDeclArgs, HostTraitDeclConfig, HostTraitMethodConfig,
};
use darling::FromMeta;
use darling::ast::NestedMeta;
use quote::format_ident;
use syn::{
    Attribute, Expr, FnArg, Ident, LitStr, Meta, Path, Token, Type, TypeParamBound, parse::Parser,
    punctuated::Punctuated,
};

pub(super) fn collect_capability_paths(
    bounds: &Punctuated<TypeParamBound, Token![+]>,
) -> syn::Result<Vec<Path>> {
    let mut paths = Vec::new();
    for bound in bounds {
        let TypeParamBound::Trait(bound) = bound else {
            return Err(syn::Error::new_spanned(
                bound,
                "#[qt(host(...))] trait supertraits must be capability trait paths",
            ));
        };
        paths.push(bound.path.clone());
    }
    Ok(paths)
}

pub(super) fn parse_trait_host_decl_attr(
    attr: proc_macro2::TokenStream,
) -> syn::Result<HostTraitDeclConfig> {
    let meta_items = Punctuated::<syn::Meta, Token![,]>::parse_terminated.parse(attr.into())?;
    let meta_items = meta_items.into_iter().collect::<Vec<_>>();
    let mut filtered = Vec::new();

    for meta in meta_items {
        match meta {
            syn::Meta::List(list) if list.path.is_ident("use") => {
                return Err(syn::Error::new_spanned(
                    list,
                    "use(...) removed; express host capability composition with Rust supertraits",
                ));
            }
            syn::Meta::NameValue(meta) if meta.path.is_ident("provider") => {
                return Err(syn::Error::new_spanned(
                    meta,
                    "provider = ... removed; carry direct host facts such as layout = ... instead",
                ));
            }
            other => filtered.push(other),
        }
    }

    let nested = filtered
        .iter()
        .cloned()
        .map(NestedMeta::Meta)
        .collect::<Vec<_>>();
    let args = HostTraitDeclArgs::from_list(&nested)
        .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;
    let default_layout = args.layout.map(|layout| layout.0);

    let host = match (args.class, args.include, args.factory, args.top_level) {
        (None, None, None, false) => None,
        (Some(class), Some(include), factory, top_level) => Some(HostConfig {
            class,
            include,
            factory,
            top_level,
        }),
        _ => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[qt(host)] trait declarations require both class = ... and include = ... when declaring a base host",
            ));
        }
    };

    Ok(HostTraitDeclConfig {
        host,
        default_layout,
    })
}

pub(super) fn parse_lit_str_expr(expr: Expr, message: &str) -> syn::Result<LitStr> {
    let Expr::Lit(expr_lit) = expr else {
        return Err(syn::Error::new_spanned(expr, message));
    };
    let syn::Lit::Str(value) = expr_lit.lit else {
        return Err(syn::Error::new_spanned(expr_lit, message));
    };
    Ok(value)
}

pub(super) fn host_method_attr_parts(attrs: &[Attribute]) -> syn::Result<HostMethodAttrParts> {
    let mut nested = Vec::new();
    let mut exports = Vec::new();
    let mut includes = Vec::new();

    for attr in attrs {
        if !attr.path().is_ident("qt") {
            continue;
        }
        let list = attr.meta.require_list()?;
        for item in NestedMeta::parse_meta_list(list.tokens.clone())? {
            match &item {
                NestedMeta::Meta(Meta::NameValue(meta)) if meta.path.is_ident("export") => {
                    exports.push(parse_lit_str_expr(
                        meta.value.clone(),
                        "#[qt(export = ...)] on #[qt(host)] traits only accepts string export names",
                    )?);
                }
                NestedMeta::Meta(Meta::NameValue(meta)) if meta.path.is_ident("include") => {
                    includes.push(parse_lit_str_expr(
                        meta.value.clone(),
                        "#[qt(include = ...)] expects string include path",
                    )?);
                }
                _ => nested.push(item),
            }
        }
    }

    let args = HostMethodAttrArgs::from_list(&nested)
        .map_err(|error| syn::Error::new(proc_macro2::Span::call_site(), error.to_string()))?;

    Ok(HostMethodAttrParts {
        args,
        exports,
        includes,
    })
}

pub(super) fn host_decl_helper_path(
    trait_path: &Path,
    kind: HostDeclHelperKind,
) -> syn::Result<Path> {
    let mut helper_path = trait_path.clone();
    let Some(last) = helper_path.segments.last_mut() else {
        return Err(syn::Error::new_spanned(
            trait_path,
            "#[qt(host)] requires a named trait path",
        ));
    };
    last.ident = helper_ident_for_trait(&last.ident, kind);
    Ok(helper_path)
}

pub(super) fn helper_ident_for_trait(trait_ident: &Ident, kind: HostDeclHelperKind) -> Ident {
    match kind {
        HostDeclHelperKind::Spec => format_ident!("QtHostSpecDecl{}", trait_ident),
        HostDeclHelperKind::Codegen => format_ident!("QtHostCodegenDecl{}", trait_ident),
        HostDeclHelperKind::Runtime => format_ident!("QtHostRuntimeDecl{}", trait_ident),
    }
}

pub(super) fn parse_host_trait_method_config(
    sig: &mut syn::Signature,
    attrs: &[Attribute],
) -> syn::Result<HostTraitMethodConfig> {
    let parts = host_method_attr_parts(attrs)?;
    let args = parts.args;
    if args.is_const && args.command {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[qt(const)] and #[qt(command)] are mutually exclusive",
        ));
    }
    let notify_label = args.notify.map(|notify| notify.0);
    let qt_signal_name = args.qt_signal.map(|qt_signal_name| qt_signal_name.value());
    let exports = parts
        .exports
        .into_iter()
        .map(|export_name| export_name.value())
        .collect::<Vec<_>>();
    let signature = args.signature.map(|signature| signature.value());
    let extra_includes = parts.includes;
    let setter = args.setter;
    let getter = args.getter;
    let behavior = if args.is_const {
        HostPropBehavior::Const
    } else if args.command {
        HostPropBehavior::Command
    } else {
        HostPropBehavior::State
    };
    let default = args.default.map(|default| default.0);
    let prop_spec = args.prop.map(|prop_name| HostPropSpecConfig {
        rust_name: prop_name.0.rust_name,
        js_name: prop_name.0.js_name,
        behavior,
        default: default.clone(),
    });

    let echoes = parse_host_event_echoes(sig)?;

    if let Some(label) = notify_label {
        for export in &exports {
            if exports
                .iter()
                .filter(|candidate| *candidate == export)
                .count()
                > 1
            {
                return Err(syn::Error::new_spanned(
                    sig,
                    format!(
                        "duplicate #[qt(export = ...)] target {} in host notify method",
                        export
                    ),
                ));
            }
        }
        if prop_spec.is_some() || setter || getter || signature.is_some() || default.is_some() {
            return Err(syn::Error::new_spanned(
                sig,
                "#[qt(notify = ...)] methods do not accept prop metadata, setter/getter, or signature",
            ));
        }
        super::validate_host_event_method(sig)?;
        return Ok(match qt_signal_name {
            Some(qt_signal_name) => HostTraitMethodConfig::Signal(HostBehaviorEventConfig {
                lower_name: label.clone(),
                label,
                qt_signal_name: Some(qt_signal_name),
                exports,
                echoes,
                extra_includes,
            }),
            None => HostTraitMethodConfig::Event(HostBehaviorEventConfig {
                lower_name: label.clone(),
                label,
                qt_signal_name: None,
                exports,
                echoes,
                extra_includes,
            }),
        });
    }

    if !echoes.is_empty() {
        return Err(syn::Error::new_spanned(
            sig,
            "#[qt(echo(...))] is only supported on #[qt(notify = ...)] methods",
        ));
    }

    if !exports.is_empty() {
        return Err(syn::Error::new_spanned(
            sig,
            "#[qt(export = ...)] is only supported on #[qt(notify = ...)] methods",
        ));
    }

    if signature.is_some() {
        if prop_spec.is_some()
            || setter
            || getter
            || qt_signal_name.is_some()
            || !exports.is_empty()
            || default.is_some()
        {
            return Err(syn::Error::new_spanned(
                sig,
                "#[qt(host)] override-like methods only accept signature/include metadata",
            ));
        }
        super::validate_host_override_target_method(sig)?;
        return Ok(HostTraitMethodConfig::Override(
            super::model::HostBehaviorOverrideConfig {
                signature: signature.expect("checked signature"),
                extra_includes,
            },
        ));
    }

    if let Some(prop) = prop_spec {
        return Ok(HostTraitMethodConfig::Codegen(
            parse_host_prop_codegen_config(sig, prop, extra_includes, setter, getter)?,
        ));
    }

    Err(syn::Error::new_spanned(
        sig,
        "#[qt(host)] methods require #[qt(notify = ...)], #[qt(signature = ...)], or #[qt(prop = ...)]",
    ))
}

pub(super) fn parse_host_prop_codegen_config(
    sig: &syn::Signature,
    prop: HostPropSpecConfig,
    extra_includes: Vec<LitStr>,
    setter: bool,
    getter: bool,
) -> syn::Result<HostCodegenMethodConfig> {
    let prop_lower_name = super::internal_host_prop_lower_name(&prop.js_name);
    let kind = if setter == getter {
        return Err(syn::Error::new_spanned(
            sig,
            "#[qt(host)] prop exports require exactly one of #[qt(setter)] or #[qt(getter)]",
        ));
    } else if setter {
        let (arg_name, value_type) = super::validate_host_prop_setter_method(sig)?;
        HostCodegenMethodKind::PropSetter {
            prop_lower_name,
            arg_name,
            value_type,
        }
    } else {
        let value_type = super::validate_host_prop_getter_method(sig)?;
        HostCodegenMethodKind::PropGetter {
            prop_lower_name,
            value_type,
        }
    };

    Ok(HostCodegenMethodConfig {
        kind,
        extra_includes,
        prop: Some(prop),
    })
}

pub(super) fn host_behavior_args(
    sig: &syn::Signature,
) -> syn::Result<Vec<(&Ident, &Type, String)>> {
    sig.inputs
        .iter()
        .skip(1)
        .map(|arg| {
            let FnArg::Typed(arg) = arg else {
                unreachable!();
            };
            let syn::Pat::Ident(pat_ident) = arg.pat.as_ref() else {
                return Err(syn::Error::new_spanned(
                    &arg.pat,
                    "#[qt(host)] behavior method arguments require simple identifiers",
                ));
            };
            if crate::common::option_inner_type(&arg.ty).is_some() {
                return Err(syn::Error::new_spanned(
                    &arg.ty,
                    "#[qt(host)] behavior methods do not support Option<_> arguments",
                ));
            }
            if super::unwrap_result_type(&arg.ty)?.is_some() {
                return Err(syn::Error::new_spanned(
                    &arg.ty,
                    "#[qt(host)] behavior methods do not support Result<_> arguments",
                ));
            }
            Ok((
                &pat_ident.ident,
                arg.ty.as_ref(),
                crate::common::snake_to_lower_camel(&pat_ident.ident.to_string()),
            ))
        })
        .collect()
}

pub(super) fn parse_host_event_echoes(
    sig: &mut syn::Signature,
) -> syn::Result<Vec<super::model::HostEventEchoConfig>> {
    let payload_count = sig.inputs.len().saturating_sub(1);
    let mut echoes = Vec::new();

    for arg in sig.inputs.iter_mut().skip(1) {
        let FnArg::Typed(arg) = arg else {
            continue;
        };
        let syn::Pat::Ident(pat_ident) = arg.pat.as_ref() else {
            continue;
        };
        let js_name = crate::common::snake_to_lower_camel(&pat_ident.ident.to_string());
        let value_path = if payload_count == 1 {
            String::new()
        } else {
            js_name.clone()
        };

        let mut retained_attrs = Vec::new();
        for attr in std::mem::take(&mut arg.attrs) {
            if !attr.path().is_ident("qt") {
                retained_attrs.push(attr);
                continue;
            }

            let list = attr.meta.require_list()?;
            let mut attr_echoes = Vec::new();
            for item in NestedMeta::parse_meta_list(list.tokens.clone())? {
                let NestedMeta::Meta(meta) = &item else {
                    return Err(syn::Error::new(
                        proc_macro2::Span::call_site(),
                        "unsupported #[qt(...)] parameter option",
                    ));
                };
                if !meta.path().is_ident("echo") {
                    return Err(syn::Error::new_spanned(
                        meta,
                        "unsupported #[qt(...)] parameter option",
                    ));
                }
                let echo = EchoDirectiveArg::from_nested_meta(&item).map_err(|error| {
                    syn::Error::new(proc_macro2::Span::call_site(), error.to_string())
                })?;
                attr_echoes.push(super::model::HostEventEchoConfig {
                    prop_js_name: echo.prop_js_name.unwrap_or_else(|| js_name.clone()),
                    value_path: value_path.clone(),
                });
            }
            if attr_echoes.is_empty() {
                retained_attrs.push(attr);
            } else {
                echoes.extend(attr_echoes);
            }
        }
        arg.attrs = retained_attrs;
    }

    for echo in &echoes {
        if echoes
            .iter()
            .filter(|candidate| candidate.prop_js_name == echo.prop_js_name)
            .count()
            > 1
        {
            return Err(syn::Error::new_spanned(
                sig,
                format!(
                    "duplicate #[qt(echo(...))] target {} in host notify method",
                    echo.prop_js_name
                ),
            ));
        }
    }

    Ok(echoes)
}

pub(super) fn parse_host_config(
    sig: &syn::Signature,
    attrs: &[Attribute],
) -> syn::Result<HostCodegenMethodConfig> {
    let parts = host_method_attr_parts(attrs)?;
    let args = parts.args;
    if args.notify.is_some() || args.qt_signal.is_some() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[qt(host)] inherent methods do not accept #[qt(notify = ...)] or #[qt(qt_signal = ...)]",
        ));
    }
    if !parts.exports.is_empty() {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[qt(export = ...)] removed; use #[qt(prop = ...)]",
        ));
    }
    if args.is_const && args.command {
        return Err(syn::Error::new(
            proc_macro2::Span::call_site(),
            "#[qt(const)] and #[qt(command)] are mutually exclusive",
        ));
    }

    let behavior = if args.is_const {
        HostPropBehavior::Const
    } else if args.command {
        HostPropBehavior::Command
    } else {
        HostPropBehavior::State
    };
    let default = args.default.map(|default| default.0);
    let prop = args.prop.map(|prop_name| HostPropSpecConfig {
        rust_name: prop_name.0.rust_name,
        js_name: prop_name.0.js_name,
        behavior,
        default: default.clone(),
    });
    let signature = args.signature.map(|signature| signature.value());
    let extra_includes = parts.includes;
    let setter = args.setter;
    let getter = args.getter;

    let kind = match (prop.clone(), signature) {
        (Some(prop_spec), None) => {
            let prop_lower_name = super::internal_host_prop_lower_name(&prop_spec.js_name);
            if setter == getter {
                return Err(syn::Error::new(
                    proc_macro2::Span::call_site(),
                    "#[qt(host)] prop methods require exactly one of #[qt(setter)] or #[qt(getter)]",
                ));
            }

            if setter {
                let (arg_name, value_type) = super::validate_host_prop_setter_method(sig)?;
                HostCodegenMethodKind::PropSetter {
                    prop_lower_name,
                    arg_name,
                    value_type,
                }
            } else {
                let value_type = super::validate_host_prop_getter_method(sig)?;
                HostCodegenMethodKind::PropGetter {
                    prop_lower_name,
                    value_type,
                }
            }
        }
        (Some(_), Some(_)) => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[qt(host)] prop methods do not accept #[qt(signature = ...)]",
            ));
        }
        (None, _) => {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                "#[qt(host)] methods require #[qt(prop = ...)]",
            ));
        }
    };

    Ok(HostCodegenMethodConfig {
        kind,
        extra_includes,
        prop,
    })
}
