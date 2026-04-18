mod model;
mod parse;

pub(crate) use self::model::{
    ConstructorSpec, GetterRuntimeMethod, ManualPropEntry, ManualPropKind, QtMethodImplKind,
    QtPropMethodConfig, SetterRuntimeMethod,
};
pub(crate) use self::parse::{
    classify_qt_method_impl, method_uses_qt_prop_attrs, parse_constructor_params,
    parse_qt_prop_method_config, render_constructor_param_expr, validate_constructor_signature,
    validate_qt_prop_getter_signature, validate_qt_prop_setter_signature,
};
use crate::common::{
    option_inner_type, widget_core_decl_path, widget_core_runtime_path, widget_core_schema_path,
};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::{Expr, FnArg, ItemImpl, Lit, LitStr, ReturnType, Type};

pub(crate) fn expand_qt_prop_methods_impl(
    input: ItemImpl,
) -> syn::Result<proc_macro2::TokenStream> {
    let schema = widget_core_schema_path();
    let runtime = widget_core_runtime_path();
    let decl = widget_core_decl_path();
    expand_qt_prop_methods_impl_with_paths(input, &schema, &runtime, &decl)
}

fn expand_qt_prop_methods_impl_with_paths(
    mut input: ItemImpl,
    schema: &proc_macro2::TokenStream,
    runtime: &proc_macro2::TokenStream,
    decl: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    if input.trait_.is_some() {
        return Err(syn::Error::new_spanned(
            input.self_ty.as_ref(),
            "#[qt_methods] only supports inherent impl blocks",
        ));
    }

    let self_ty = input.self_ty.clone();
    let impl_generics = input.generics.clone();
    let (_, _, where_clause) = input.generics.split_for_impl();
    let mut retained_items = Vec::new();
    let mut constructor = None::<ConstructorSpec>;
    let mut prop_entries = BTreeMap::<String, ManualPropEntry>::new();
    let mut next_setter_slot = 0u16;
    let mut next_getter_slot = 0u16;

    for mut item in std::mem::take(&mut input.items) {
        let syn::ImplItem::Fn(method) = &mut item else {
            return Err(syn::Error::new_spanned(
                item,
                "#[qt_methods] only supports methods",
            ));
        };

        let config = parse_qt_prop_method_config(&method.attrs)?;
        method.attrs.retain(|attr| !attr.path().is_ident("qt"));

        match config {
            QtPropMethodConfig::Plain => retained_items.push(item),
            QtPropMethodConfig::Constructor => {
                let params = parse_constructor_params(&mut method.sig)?;
                validate_constructor_signature(&method.sig)?;
                if constructor
                    .replace(ConstructorSpec {
                        ident: method.sig.ident.clone(),
                        returns_result: unwrap_result_type_from_output(&method.sig.output)?
                            .is_some(),
                        params,
                    })
                    .is_some()
                {
                    return Err(syn::Error::new_spanned(
                        &method.sig.ident,
                        "#[qt_methods] only allows one #[qt(constructor)] method",
                    ));
                }
                retained_items.push(item);
            }
            QtPropMethodConfig::Prop(prop) => {
                let prop_type = match prop.kind {
                    ManualPropKind::Setter => qt_prop_setter_value_type(&method.sig)?,
                    ManualPropKind::Getter => qt_prop_getter_value_type(&method.sig)?,
                };
                let entry = prop_entries
                    .entry(prop.js_name.clone())
                    .or_insert_with(|| ManualPropEntry::new(&prop.js_name));
                entry.record_value_type(&prop_type)?;
                if let Some(default) = prop.default.as_ref() {
                    if entry.default.is_some() {
                        return Err(syn::Error::new_spanned(
                            default,
                            "duplicate widget prop default",
                        ));
                    }
                    entry.default = Some(spec_default_value_tokens(
                        &schema,
                        &prop_type,
                        Some(default),
                    )?);
                }
                match prop.kind {
                    ManualPropKind::Setter => {
                        let slot_ref = if prop.init {
                            &mut entry.init_setter_slot
                        } else {
                            &mut entry.setter_slot
                        };
                        if slot_ref.is_some() {
                            return Err(syn::Error::new_spanned(
                                &method.sig.ident,
                                if prop.init {
                                    "duplicate widget prop init setter for same prop"
                                } else {
                                    "duplicate widget prop live setter for same prop"
                                },
                            ));
                        }
                        validate_qt_prop_setter_signature(&method.sig)?;
                        let slot = next_setter_slot;
                        next_setter_slot = next_setter_slot.checked_add(1).ok_or_else(|| {
                            syn::Error::new_spanned(
                                &method.sig.ident,
                                "#[qt_methods] supports at most 65535 widget prop setters",
                            )
                        })?;
                        *slot_ref = Some(slot);
                        entry.value_type = Some(qt_prop_value_type_tokens(&schema, &prop_type)?);
                        let runtime_method = SetterRuntimeMethod {
                            ident: method.sig.ident.clone(),
                            value_type: prop_type.clone(),
                            returns_result: qt_prop_setter_returns_result(&method.sig)?,
                        };
                        if prop.init {
                            entry.init_setter_runtime = Some(runtime_method);
                        } else {
                            entry.setter_runtime = Some(runtime_method);
                        }
                    }
                    ManualPropKind::Getter => {
                        if entry.getter_slot.is_some() {
                            return Err(syn::Error::new_spanned(
                                &method.sig.ident,
                                "duplicate widget prop getter for same prop",
                            ));
                        }
                        validate_qt_prop_getter_signature(&method.sig)?;
                        let slot = next_getter_slot;
                        next_getter_slot = next_getter_slot.checked_add(1).ok_or_else(|| {
                            syn::Error::new_spanned(
                                &method.sig.ident,
                                "#[qt_methods] supports at most 65535 widget prop getters",
                            )
                        })?;
                        entry.getter_slot = Some(slot);
                        entry.value_type = Some(qt_prop_value_type_tokens(&schema, &prop_type)?);
                        entry.getter_runtime = Some(GetterRuntimeMethod {
                            ident: method.sig.ident.clone(),
                            value_type: prop_type.clone(),
                            returns_result: qt_prop_getter_returns_result(&method.sig)?,
                        });
                    }
                }

                retained_items.push(item);
            }
        }
    }

    input.items = retained_items;

    let prop_specs = prop_entries
        .values()
        .map(|entry| entry.spec_tokens(&schema))
        .collect::<syn::Result<Vec<_>>>()?;
    let manual_props = prop_entries.values().collect::<Vec<_>>();
    let spec_key_tokens = quote!(#decl::SpecWidgetKey::new(concat!(
        module_path!(),
        "::",
        stringify!(#self_ty)
    )));
    let prop_runtime_fragment =
        build_widget_prop_runtime_fragment(&runtime, &self_ty, &decl, manual_props.as_slice())?;
    let create_prop_specs = constructor
        .as_ref()
        .map(|constructor| {
            constructor
                .params
                .iter()
                .map(|param| {
                    let key = LitStr::new(&param.js_name, proc_macro2::Span::call_site());
                    let value_ty = &param.value_ty;
                    Ok(quote! {
                        #schema::SpecCreateProp {
                            key: #key,
                            value_type: <#value_ty as #schema::QtType>::INFO,
                        }
                    })
                })
                .collect::<syn::Result<Vec<_>>>()
        })
        .transpose()?
        .unwrap_or_default();

    let create_instance = if let Some(constructor) = constructor.as_ref() {
        let factory_name = format_ident!("__qt_prop_create_instance");
        let constructor_ident = &constructor.ident;
        let constructor_runtime = runtime.clone();
        let constructor_args = constructor
            .params
            .iter()
            .map(|param| render_constructor_param_expr(param, &constructor_runtime))
            .collect::<syn::Result<Vec<_>>>()?;
        let construct_widget = if constructor.returns_result {
            quote!(#self_ty::#constructor_ident(#(#constructor_args),*)?)
        } else {
            quote!(#self_ty::#constructor_ident(#(#constructor_args),*))
        };
        Some(quote! {
            fn #factory_name(
                handle: #runtime::WidgetHandle,
                create_props: &[#runtime::WidgetCreateProp],
            ) -> #runtime::WidgetResult<std::sync::Arc<dyn #runtime::QtWidgetInstanceDyn>> {
                let widget = #construct_widget;
                Ok(#runtime::new_widget_instance::<#self_ty, #schema::NoMethods>(
                    handle,
                    widget,
                    #runtime::resolve_widget_host_behavior(
                        <#self_ty as #runtime::QtWidgetNativeDecl>::NATIVE_DECL.spec_key
                    ),
                    #runtime::resolve_widget_prop_runtime(
                        <#self_ty as #runtime::QtWidgetNativeDecl>::NATIVE_DECL.spec_key
                    ),
                ))
            }
        })
    } else {
        None
    };

    let create_instance_ref = if constructor.is_some() {
        quote!(Some(__qt_prop_create_instance))
    } else {
        quote!(None)
    };
    let prop_decl_fragment = if prop_specs.is_empty() {
        None
    } else {
        Some(quote! {
            const _: () = {
                use #runtime::linkme::distributed_slice;

                fn __qt_widget_prop_decl() -> &'static [#schema::SpecPropDecl] {
                    static DECL: &[#schema::SpecPropDecl] = &[
                        #(#prop_specs,)*
                    ];

                    DECL
                }

                #[distributed_slice(#runtime::QT_WIDGET_PROP_DECL_FRAGMENTS)]
                #[linkme(crate = #runtime::linkme)]
                static __QT_WIDGET_PROP_FRAGMENT: &#runtime::WidgetPropDeclFragment =
                    &#runtime::WidgetPropDeclFragment {
                        spec_key: #spec_key_tokens,
                        decl: __qt_widget_prop_decl,
                    };
            };
        })
    };
    let widget_decl_assert = quote! {
        const _: fn() = || {
            fn __qt_assert_widget_decl<T: #schema::QtWidgetDecl>() {}
            __qt_assert_widget_decl::<#self_ty>();
        };
    };

    Ok(quote! {
        #input

        #widget_decl_assert

        #create_instance

        impl #impl_generics #runtime::QtWidgetPropDecl for #self_ty #where_clause {
            const PROP_DECL: #runtime::WidgetPropDecl = #runtime::WidgetPropDecl {
                spec_key: #spec_key_tokens,
                create_instance: #create_instance_ref,
                create_props: &[
                    #(#create_prop_specs,)*
                ],
                props: &[
                    #(#prop_specs,)*
                ],
            };
        }

        const _: () = {
            use #runtime::linkme::distributed_slice;

            #[distributed_slice(#runtime::QT_WIDGET_PROP_DECLS)]
            #[linkme(crate = #runtime::linkme)]
            static __QT_WIDGET_PROP_DECL: &#runtime::WidgetPropDecl =
                &<#self_ty as #runtime::QtWidgetPropDecl>::PROP_DECL;
        };

        #prop_decl_fragment
        #prop_runtime_fragment
    })
}

pub(crate) fn build_widget_prop_runtime_fragment(
    runtime: &proc_macro2::TokenStream,
    self_ty: &Type,
    decl: &proc_macro2::TokenStream,
    props: &[&ManualPropEntry],
) -> syn::Result<proc_macro2::TokenStream> {
    let mut setter_helpers = Vec::new();
    let mut setter_meta = Vec::new();
    let mut getter_helpers = Vec::new();
    let mut getter_meta = Vec::new();

    for prop in props {
        if let Some(method) = prop
            .setter_runtime
            .as_ref()
            .or(prop.init_setter_runtime.as_ref())
        {
            let (helper, meta) =
                render_setter_runtime_meta(runtime, self_ty, &prop.js_name, method)?;
            setter_helpers.push(helper);
            setter_meta.push(meta);
        }

        if let Some(method) = prop.getter_runtime.as_ref() {
            let (helper, meta) =
                render_getter_runtime_meta(runtime, self_ty, &prop.js_name, method)?;
            getter_helpers.push(helper);
            getter_meta.push(meta);
        }
    }

    if setter_meta.is_empty() && getter_meta.is_empty() {
        return Ok(proc_macro2::TokenStream::new());
    }

    Ok(quote! {
        const _: () = {
            use #runtime::linkme::distributed_slice;

            #(#setter_helpers)*
            #(#getter_helpers)*

            fn __qt_widget_prop_runtime_decl() -> &'static #runtime::WidgetPropRuntimeDecl {
                static DECL: #runtime::WidgetPropRuntimeDecl =
                    #runtime::WidgetPropRuntimeDecl {
                        setters: &[
                            #(#setter_meta,)*
                        ],
                        getters: &[
                            #(#getter_meta,)*
                        ],
                    };

                &DECL
            }

            #[distributed_slice(#runtime::QT_WIDGET_PROP_RUNTIME_FRAGMENTS)]
            #[linkme(crate = #runtime::linkme)]
            static __QT_WIDGET_PROP_RUNTIME_FRAGMENT: &#runtime::WidgetPropRuntimeFragment =
                &#runtime::WidgetPropRuntimeFragment {
                    spec_key: #decl::SpecWidgetKey::new(concat!(
                        module_path!(),
                        "::",
                        stringify!(#self_ty)
                    )),
                    decl: __qt_widget_prop_runtime_decl,
                };
        };
    })
}

fn render_setter_runtime_meta(
    runtime: &proc_macro2::TokenStream,
    self_ty: &Type,
    js_name: &str,
    method: &SetterRuntimeMethod,
) -> syn::Result<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
    let helper_name = format_ident!(
        "__qt_prop_setter_{}",
        method.ident,
        span = method.ident.span()
    );
    let method_ident = &method.ident;
    let js_name = LitStr::new(js_name, proc_macro2::Span::call_site());
    let ty = option_inner_type(&method.value_type).unwrap_or(&method.value_type);
    let body = if method.returns_result {
        quote! {
            let widget = unsafe { &mut *(raw.cast::<#self_ty>()) };
            let value = <#ty as #runtime::TryFromQt>::try_from_qt(value)?;
            #self_ty::#method_ident(widget, value)
        }
    } else {
        quote! {
            let widget = unsafe { &mut *(raw.cast::<#self_ty>()) };
            let value = <#ty as #runtime::TryFromQt>::try_from_qt(value)?;
            #self_ty::#method_ident(widget, value);
            Ok(())
        }
    };

    Ok((
        quote! {
            unsafe fn #helper_name(
                raw: *mut (),
                value: #runtime::QtValue,
            ) -> #runtime::WidgetResult<()> {
                #body
            }
        },
        quote! {
            #runtime::WidgetPropSetterRuntimeMeta {
                js_name: #js_name,
                invoke: #helper_name,
            }
        },
    ))
}

fn render_getter_runtime_meta(
    runtime: &proc_macro2::TokenStream,
    self_ty: &Type,
    js_name: &str,
    method: &GetterRuntimeMethod,
) -> syn::Result<(proc_macro2::TokenStream, proc_macro2::TokenStream)> {
    let helper_name = format_ident!(
        "__qt_prop_getter_{}",
        method.ident,
        span = method.ident.span()
    );
    let method_ident = &method.ident;
    let js_name = LitStr::new(js_name, proc_macro2::Span::call_site());
    let ty = option_inner_type(&method.value_type).unwrap_or(&method.value_type);
    let body = if method.returns_result {
        quote! {
            let widget = unsafe { &*(raw.cast::<#self_ty>()) };
            let value = #self_ty::#method_ident(widget)?;
            <#ty as #runtime::IntoQt>::into_qt(value)
        }
    } else {
        quote! {
            let widget = unsafe { &*(raw.cast::<#self_ty>()) };
            let value = #self_ty::#method_ident(widget);
            <#ty as #runtime::IntoQt>::into_qt(value)
        }
    };

    Ok((
        quote! {
            unsafe fn #helper_name(raw: *const ()) -> #runtime::WidgetResult<#runtime::QtValue> {
                #body
            }
        },
        quote! {
            #runtime::WidgetPropGetterRuntimeMeta {
                js_name: #js_name,
                invoke: #helper_name,
            }
        },
    ))
}

fn method_return_type(output: &ReturnType) -> syn::Result<Option<&Type>> {
    let ty = match output {
        ReturnType::Default => return Ok(None),
        ReturnType::Type(_, ty) => ty.as_ref(),
    };
    let Some(inner) = unwrap_result_type(ty)? else {
        return Ok(Some(ty));
    };
    Ok(Some(inner))
}

pub(crate) fn qt_prop_setter_value_type(sig: &syn::Signature) -> syn::Result<Type> {
    let FnArg::Typed(arg) = sig.inputs.iter().nth(1).ok_or_else(|| {
        syn::Error::new_spanned(sig, "widget prop setters require one value argument")
    })?
    else {
        unreachable!()
    };
    Ok((*arg.ty).clone())
}

pub(crate) fn qt_prop_getter_value_type(sig: &syn::Signature) -> syn::Result<Type> {
    method_return_type(&sig.output)?.cloned().ok_or_else(|| {
        syn::Error::new_spanned(&sig.output, "widget prop getters must return a value")
    })
}

pub(crate) fn qt_prop_setter_returns_result(sig: &syn::Signature) -> syn::Result<bool> {
    Ok(unwrap_result_type_from_output(&sig.output)?.is_some())
}

pub(crate) fn qt_prop_getter_returns_result(sig: &syn::Signature) -> syn::Result<bool> {
    Ok(unwrap_result_type_from_output(&sig.output)?.is_some())
}

fn unwrap_result_type_from_output(output: &ReturnType) -> syn::Result<Option<&Type>> {
    match output {
        ReturnType::Default => Ok(None),
        ReturnType::Type(_, ty) => unwrap_result_type(ty),
    }
}

pub(crate) fn qt_prop_value_type_tokens(
    schema: &proc_macro2::TokenStream,
    ty: &Type,
) -> syn::Result<proc_macro2::TokenStream> {
    let ty = option_inner_type(ty).unwrap_or(ty);
    Ok(quote!(<#ty as #schema::QtType>::INFO))
}

fn unwrap_result_type(ty: &Type) -> syn::Result<Option<&Type>> {
    let Type::Path(type_path) = ty else {
        return Ok(None);
    };
    let Some(segment) = type_path.path.segments.last() else {
        return Ok(None);
    };
    if segment.ident != "Result" && segment.ident != "WidgetResult" {
        return Ok(None);
    }
    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(syn::Error::new_spanned(
            &segment.arguments,
            "Result return type requires type arguments",
        ));
    };
    let Some(first) = arguments.args.first() else {
        return Err(syn::Error::new_spanned(
            arguments,
            "Result return type requires an Ok value type",
        ));
    };
    let syn::GenericArgument::Type(inner) = first else {
        return Err(syn::Error::new_spanned(first, "unsupported Result Ok type"));
    };
    Ok(Some(inner))
}

fn is_unit_type(ty: &Type) -> bool {
    matches!(ty, Type::Tuple(tuple) if tuple.elems.is_empty())
}

fn slot_literal(slot: Option<u16>) -> proc_macro2::TokenStream {
    match slot {
        Some(slot) => quote!(Some(#slot)),
        None => quote!(None),
    }
}

pub(crate) fn spec_default_value_tokens(
    schema: &proc_macro2::TokenStream,
    ty: &Type,
    default: Option<&Expr>,
) -> syn::Result<proc_macro2::TokenStream> {
    let value_ty = option_inner_type(ty).unwrap_or(ty);
    let Some(default) = default else {
        return Ok(quote!(#schema::SpecPropDefaultValue::None));
    };
    let value = default;
    match value {
        Expr::Lit(expr_lit) => match &expr_lit.lit {
            Lit::Bool(value) => Ok(quote!(#schema::SpecPropDefaultValue::Bool(#value))),
            Lit::Int(value) => {
                let value = value.base10_parse::<i32>()?;
                Ok(quote!(#schema::SpecPropDefaultValue::I32(#value)))
            }
            Lit::Float(value) => {
                let value = value.base10_parse::<f64>()?;
                Ok(quote!(#schema::SpecPropDefaultValue::F64(#value)))
            }
            Lit::Str(value) => Ok(quote!(#schema::SpecPropDefaultValue::String(#value))),
            other => Err(syn::Error::new_spanned(
                other,
                "unsupported exported prop default literal",
            )),
        },
        Expr::Path(path) => {
            let variant = path
                .path
                .segments
                .last()
                .ok_or_else(|| syn::Error::new_spanned(path, "expected enum path"))?
                .ident
                .to_string();
            let variant = pascal_to_kebab(&variant);
            Ok(quote!(#schema::SpecPropDefaultValue::Enum(#variant)))
        }
        Expr::Call(call) => {
            if call.args.len() != 1 {
                return Err(syn::Error::new_spanned(
                    call,
                    "unsupported exported prop default expression",
                ));
            }
            spec_default_value_tokens(schema, value_ty, call.args.first())
        }
        other => Err(syn::Error::new_spanned(
            other,
            "unsupported exported prop default expression",
        )),
    }
}

fn pascal_to_kebab(value: &str) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if ch.is_uppercase() {
            if index != 0 {
                out.push('-');
            }
            out.extend(ch.to_lowercase());
        } else {
            out.push(ch);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::{
        ManualPropKind, QtMethodImplKind, QtPropMethodConfig, classify_qt_method_impl,
        expand_qt_prop_methods_impl_with_paths, parse_constructor_params,
        parse_qt_prop_method_config, render_constructor_param_expr,
    };
    use quote::quote;
    use syn::parse_quote;

    fn parse_prop_config(
        attrs: Vec<syn::Attribute>,
    ) -> syn::Result<(String, ManualPropKind, bool)> {
        match parse_qt_prop_method_config(&attrs)? {
            QtPropMethodConfig::Prop(prop) => Ok((prop.js_name, prop.kind, prop.init)),
            QtPropMethodConfig::Plain => panic!("expected prop config"),
            QtPropMethodConfig::Constructor => panic!("expected prop config"),
        }
    }

    #[test]
    fn bare_setter_matches_explicit_update() {
        let bare = parse_prop_config(vec![parse_quote!(#[qt(prop(text), setter)])])
            .expect("bare setter should parse");
        let explicit = parse_prop_config(vec![parse_quote!(#[qt(prop(text), setter, update)])])
            .expect("explicit update setter should parse");

        assert_eq!(bare.0, explicit.0);
        assert!(matches!(bare.1, ManualPropKind::Setter));
        assert!(matches!(explicit.1, ManualPropKind::Setter));
        assert_eq!(bare.2, explicit.2);
        assert!(!bare.2, "bare setter should remain non-init");
    }

    #[test]
    fn namespaced_prop_names_are_rejected() {
        let equals_error =
            parse_qt_prop_method_config(&[parse_quote!(#[qt(prop = frame::seq, getter)])])
                .err()
                .expect("prop = path should fail");
        let call_error =
            parse_qt_prop_method_config(&[parse_quote!(#[qt(prop(frame::seq), getter)])])
                .err()
                .expect("prop(path) should fail");

        assert!(equals_error.to_string().contains("flat prop name"));
        assert!(call_error.to_string().contains("flat prop name"));
    }

    #[test]
    fn init_and_update_are_mutually_exclusive() {
        let error = parse_qt_prop_method_config(&[parse_quote!(
            #[qt(prop(text), setter, init, update)]
        )])
        .err()
        .expect("init and update should conflict");

        assert!(error.to_string().contains("mutually exclusive"));
    }

    #[test]
    fn classify_qt_method_impl_detects_pure_prop_block() {
        let input: syn::ItemImpl = parse_quote! {
            impl Widget {
                #[qt(constructor)]
                fn create() -> Self {
                    Self {}
                }

                #[qt(prop(frameSeq), getter)]
                fn frame_seq(&self) -> f64 {
                    0.0
                }
            }
        };

        assert!(matches!(
            classify_qt_method_impl(&input).expect("classifies"),
            QtMethodImplKind::Pure
        ));
    }

    #[test]
    fn classify_qt_method_impl_detects_mixed_block() {
        let input: syn::ItemImpl = parse_quote! {
            impl Widget {
                #[qt(constructor)]
                fn create() -> Self {
                    Self {}
                }

                #[qt(host)]
                fn focus(&self) {}
            }
        };

        assert!(matches!(
            classify_qt_method_impl(&input).expect("classifies"),
            QtMethodImplKind::Mixed
        ));
    }

    #[test]
    fn constructor_params_expand_into_create_props() {
        let mut method: syn::ImplItemFn = parse_quote! {
            fn create(
                #[qt(prop)] text: String,
                #[qt(prop = auto_focus, default = false)] auto_focus: bool,
            ) -> Self {
                Self { text, auto_focus }
            }
        };

        let params = parse_constructor_params(&mut method.sig).expect("constructor params parse");
        assert_eq!(params.len(), 2);
        assert_eq!(params[0].js_name, "text");
        assert_eq!(params[1].js_name, "autoFocus");

        let runtime = quote!(runtime);

        let text_expr = render_constructor_param_expr(&params[0], &runtime)
            .expect("text constructor arg renders")
            .to_string();
        let auto_focus_expr = render_constructor_param_expr(&params[1], &runtime)
            .expect("autoFocus constructor arg renders")
            .to_string();

        assert!(text_expr.contains("parse_widget_create_prop"));
        assert!(text_expr.contains("create_props"));
        assert!(text_expr.contains("\"text\""));
        assert!(text_expr.contains("missing constructor prop"));
        assert!(auto_focus_expr.contains("parse_widget_create_prop"));
        assert!(auto_focus_expr.contains("create_props"));
        assert!(auto_focus_expr.contains("\"autoFocus\""));
        assert!(auto_focus_expr.contains("None => false"));
    }

    #[test]
    fn constructor_params_require_qt_prop() {
        let mut method: syn::ImplItemFn = parse_quote! {
            fn create(text: String) -> Self {
                Self { text }
            }
        };

        let error = parse_constructor_params(&mut method.sig)
            .err()
            .expect("missing #[qt(prop)] must fail");

        assert!(error.to_string().contains("parameters require #[qt(prop)]"));
    }

    #[test]
    fn prop_methods_impl_emits_widget_decl_assertion() {
        let item: syn::ItemImpl = parse_quote! {
            impl DemoWidget {
                #[qt(prop = width, setter)]
                fn set_width(&mut self, value: i32) {
                    drop(value);
                }
            }
        };

        let expanded = expand_qt_prop_methods_impl_with_paths(
            item,
            &quote!(schema),
            &quote!(runtime),
            &quote!(decl),
        )
        .expect("prop methods impl expands")
        .to_string();

        assert!(expanded.contains("QtWidgetDecl"));
        assert!(expanded.contains("__qt_assert_widget_decl"));
        assert!(!expanded.contains("QT_WIDGET_PROP_SPEC_FRAGMENTS"));
    }
}
