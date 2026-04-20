mod model;
mod parse;

use self::model::{
    HostBehaviorEventConfig, HostBehaviorOverrideConfig, HostCodegenMethodKind, HostConfig,
    HostDeclHelperKind, HostPropBehavior, HostPropRecord, HostPropSpecConfig, HostTraitDeclConfig,
    HostTraitMethodConfig,
};
use self::parse::{
    collect_capability_paths, helper_ident_for_trait, host_behavior_args, host_decl_helper_path,
    parse_host_config, parse_host_trait_method_config, parse_trait_host_decl_attr,
};
use super::shared::{is_unit_type, qt_cpp_macro_body, unwrap_result_type};
use crate::common::{
    option_inner_type, widget_core_codegen_path, widget_core_decl_path, widget_core_runtime_path,
    widget_core_schema_path,
};
use crate::widget::emit_widget_codegen_fragment;
use quote::{format_ident, quote};
use syn::{
    Expr, FnArg, GenericParam, Ident, Item, ItemImpl, ItemTrait, LitStr, Path, ReturnType,
    TraitItem, Type, TypeParamBound, WherePredicate, punctuated::Punctuated,
};

pub(crate) fn expand_qt_host_attr(
    attr: proc_macro2::TokenStream,
    item: Item,
) -> syn::Result<proc_macro2::TokenStream> {
    match item {
        Item::Impl(input) if is_paint_impl(&input) => expand_qt_host_paint_impl(attr, input),
        Item::Impl(input) if input.trait_.is_some() => expand_qt_host_attach_impl(attr, input),
        Item::Impl(input) => expand_qt_host_inherent_impl(attr, input),
        Item::Trait(input) => expand_qt_host_trait_decl(parse_trait_host_decl_attr(attr)?, input),
        other => Err(syn::Error::new_spanned(
            other,
            "#[qt(host)] only supports traits and impl blocks",
        )),
    }
}

fn expand_qt_host_paint_impl(
    attr: proc_macro2::TokenStream,
    input: ItemImpl,
) -> syn::Result<proc_macro2::TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[qt(host)] paint impl does not accept attr arguments",
        ));
    }

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            input.generics.clone(),
            "#[qt(host)] paint impl does not support explicit impl generics",
        ));
    }

    let codegen = widget_core_codegen_path();
    let decl = widget_core_decl_path();
    let runtime = widget_core_runtime_path();
    let self_ty = input.self_ty.clone();
    let widget_ident = widget_self_ident(&self_ty)?;
    let (_, trait_path, _) = input
        .trait_
        .as_ref()
        .expect("paint impl branch requires trait path");
    let paint_impl = parse_paint_impl(trait_path)?;
    let runtime_meta = build_paint_runtime_meta(&runtime, &self_ty, &paint_impl);
    let codegen_fragment =
        paint_impl.build_codegen_fragment(&codegen, &runtime, &decl, &widget_ident)?;

    Ok(quote! {
        #input

        impl #runtime::__QtHostPaintRegistration for #self_ty {}

        const _: () = {
            use #runtime::linkme::distributed_slice;

            fn __qt_host_runtime_decl() -> &'static #runtime::HostBehaviorRuntimeDecl {
                static DECL: std::sync::OnceLock<&'static #runtime::HostBehaviorRuntimeDecl> =
                    std::sync::OnceLock::new();

                DECL.get_or_init(|| {
                    std::boxed::Box::leak(std::boxed::Box::new(
                        #runtime::HostBehaviorRuntimeDecl {
                            host_events: &#runtime::NO_WIDGET_HOST_EVENT_RUNTIME,
                            host_overrides: &#runtime::NO_WIDGET_HOST_OVERRIDE_RUNTIME,
                            paint: core::option::Option::Some(#runtime_meta),
                        }
                    ))
                })
            }

            #[distributed_slice(#runtime::QT_WIDGET_HOST_BEHAVIOR_RUNTIME_FRAGMENTS)]
            #[linkme(crate = #runtime::linkme)]
            static __QT_WIDGET_HOST_BEHAVIOR_FRAGMENT: &#runtime::WidgetHostBehaviorRuntimeFragment =
                &#runtime::WidgetHostBehaviorRuntimeFragment {
                    spec_key: #decl::SpecWidgetKey::new(concat!(
                        module_path!(),
                        "::",
                        stringify!(#widget_ident)
                    )),
                    decl: __qt_host_runtime_decl,
                };
        };

        #codegen_fragment
    })
}

fn expand_qt_host_inherent_impl(
    attr: proc_macro2::TokenStream,
    mut input: ItemImpl,
) -> syn::Result<proc_macro2::TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[qt(host)] inherent impls do not accept attr arguments",
        ));
    }

    let codegen = widget_core_codegen_path();
    let decl = widget_core_decl_path();
    let schema = widget_core_schema_path();
    let self_ty = input.self_ty.clone();
    let widget_ident = widget_self_ident(&self_ty)?;
    let mut overrides = Vec::new();
    let mut event_mounts = Vec::new();
    let mut prop_setters = Vec::new();
    let mut prop_getters = Vec::new();
    let mut prop_specs = Vec::<HostPropRecord>::new();

    for mut item in std::mem::take(&mut input.items) {
        let syn::ImplItem::Fn(method) = &mut item else {
            return Err(syn::Error::new_spanned(
                item,
                "#[qt(host)] only supports methods",
            ));
        };

        let config = parse_host_config(&method.sig, &method.attrs)?;
        method.attrs.retain(|attr| !attr.path().is_ident("qt"));
        let Some(body) = qt_cpp_macro_body(&method.block)? else {
            return Err(syn::Error::new_spanned(
                &method.block,
                "#[qt(host)] methods require a qt::cpp! body",
            ));
        };

        let rust_name = LitStr::new(&method.sig.ident.to_string(), method.sig.ident.span());
        let extra_includes = config.extra_includes.iter();

        push_host_codegen_meta(
            &codegen,
            config.kind.clone(),
            rust_name,
            extra_includes,
            body,
            &mut overrides,
            &mut event_mounts,
            &mut prop_setters,
            &mut prop_getters,
        );
        if let Some(prop) = config.prop.as_ref() {
            merge_host_prop_record(&mut prop_specs, &config.kind, prop)?;
        }
    }

    input.items.clear();
    let fragment = emit_widget_codegen_fragment(
        &codegen,
        &decl,
        &widget_ident,
        &overrides,
        &event_mounts,
        &prop_setters,
        &prop_getters,
    );
    let prop_spec_tokens = prop_specs
        .iter()
        .map(|record| build_host_prop_spec(&schema, record))
        .collect::<syn::Result<Vec<_>>>()?;
    let spec_fragment =
        emit_inherent_host_spec_fragment(&schema, &decl, &widget_ident, &prop_spec_tokens);

    Ok(quote! {
        #input

        #fragment
        #spec_fragment
    })
}

fn expand_qt_host_attach_impl(
    attr: proc_macro2::TokenStream,
    input: ItemImpl,
) -> syn::Result<proc_macro2::TokenStream> {
    if !attr.is_empty() {
        return Err(syn::Error::new_spanned(
            attr,
            "#[qt(host)] trait attachment impl does not accept attr arguments",
        ));
    }

    let codegen = widget_core_codegen_path();
    let decl = widget_core_decl_path();
    let runtime = widget_core_runtime_path();
    let schema = widget_core_schema_path();
    let self_ty = input.self_ty.clone();
    let widget_ident = widget_self_ident(&self_ty)?;
    let (_, trait_path, _) = input
        .trait_
        .as_ref()
        .expect("trait attachment impl requires trait path");
    Ok(quote! {
        #input

        const _: () = {
            use #schema::linkme::distributed_slice;

            fn __qt_host_spec_decl() -> &'static #schema::HostCapabilitySpecDecl {
                <#self_ty as #trait_path>::__qt_host_spec_decl()
            }

            #[distributed_slice(#schema::QT_WIDGET_HOST_SPEC_FRAGMENTS)]
            #[linkme(crate = #schema::linkme)]
            static __QT_WIDGET_HOST_SPEC_FRAGMENT: &#schema::WidgetHostSpecFragment =
                &#schema::WidgetHostSpecFragment {
                    spec_key: #decl::SpecWidgetKey::new(concat!(
                        module_path!(),
                        "::",
                        stringify!(#widget_ident)
                    )),
                    decl: __qt_host_spec_decl,
                };
        };

        const _: () = {
            use #codegen::linkme::distributed_slice;

            fn __qt_host_codegen_decl() -> &'static #codegen::HostCapabilityCodegenDecl {
                <#self_ty as #trait_path>::__qt_host_codegen_decl()
            }

            #[distributed_slice(#codegen::QT_WIDGET_CODEGEN_FRAGMENTS)]
            #[linkme(crate = #codegen::linkme)]
            static __QT_WIDGET_CODEGEN_FRAGMENT: &#codegen::WidgetCodegenFragment =
                &#codegen::WidgetCodegenFragment {
                    spec_key: #decl::SpecWidgetKey::new(concat!(
                        module_path!(),
                        "::",
                        stringify!(#widget_ident)
                    )),
                    decl: __qt_host_codegen_decl,
                };
        };

        const _: () = {
            use #runtime::linkme::distributed_slice;

            fn __qt_host_runtime_decl() -> &'static #runtime::HostBehaviorRuntimeDecl {
                <#self_ty as #trait_path>::__qt_host_runtime_decl()
            }

            #[distributed_slice(#runtime::QT_WIDGET_HOST_BEHAVIOR_RUNTIME_FRAGMENTS)]
            #[linkme(crate = #runtime::linkme)]
            static __QT_WIDGET_HOST_BEHAVIOR_FRAGMENT: &#runtime::WidgetHostBehaviorRuntimeFragment =
                &#runtime::WidgetHostBehaviorRuntimeFragment {
                    spec_key: #decl::SpecWidgetKey::new(concat!(
                        module_path!(),
                        "::",
                        stringify!(#widget_ident)
                    )),
                    decl: __qt_host_runtime_decl,
                };
        };
    })
}

fn is_paint_impl(input: &ItemImpl) -> bool {
    input
        .trait_
        .as_ref()
        .and_then(|(_, path, _)| path.segments.last())
        .is_some_and(|segment| segment.ident == "Paint")
}

struct PaintImplConfig {
    target_ty: Type,
    target_ctor: Path,
    kind: PaintImplKind,
}

enum PaintImplKind {
    Vello,
    QPainter,
}

impl PaintImplConfig {
    fn build_codegen_fragment(
        &self,
        codegen: &proc_macro2::TokenStream,
        runtime: &proc_macro2::TokenStream,
        decl: &proc_macro2::TokenStream,
        widget_ident: &Ident,
    ) -> syn::Result<proc_macro2::TokenStream> {
        let PaintImplKind::QPainter = self.kind else {
            return Ok(quote!());
        };

        let target_ty = &self.target_ty;
        let target_ctor = &self.target_ctor;

        Ok(quote! {
            const _: () = {
                use #codegen::linkme::distributed_slice;

                fn __qt_host_codegen_decl() -> &'static #codegen::HostCapabilityCodegenDecl {
                    static DECL: std::sync::OnceLock<&'static #codegen::HostCapabilityCodegenDecl> =
                        std::sync::OnceLock::new();

                    DECL.get_or_init(|| {
                        std::boxed::Box::leak(std::boxed::Box::new(
                            #codegen::HostCapabilityCodegenDecl {
                                host_overrides: std::boxed::Box::leak(std::boxed::Box::new(
                                    #codegen::WidgetHostOverrideCodegenSet {
                                        overrides: std::boxed::Box::leak(std::vec![
                                            #codegen::WidgetHostOverrideCodegenMeta {
                                                rust_name: "paint",
                                                target_name: "paint",
                                                opaque: <#target_ty as #runtime::QtOpaqueFacade>::INFO,
                                                bridge_fn: <#target_ctor as #codegen::QtOpaqueCodegenBridge>::HOOK_BRIDGE_FN,
                                                signature: "void paintEvent(QPaintEvent *event)",
                                                lowering: #codegen::HostCodegenLowering {
                                                    extra_includes: &["<QtGui/QPainter>"],
                                                    body: "QWidget::paintEvent(event);\nQPainter painter(&self);\ndispatch_rust(painter);\n",
                                                },
                                            }
                                        ].into_boxed_slice()),
                                    }
                                )),
                                host_event_mounts: &#codegen::NO_WIDGET_HOST_EVENT_MOUNTS,
                                host_prop_setters: &#codegen::NO_WIDGET_HOST_PROP_SETTERS,
                                host_prop_getters: &#codegen::NO_WIDGET_HOST_PROP_GETTERS,
                            }
                        ))
                    })
                }

                #[distributed_slice(#codegen::QT_WIDGET_CODEGEN_FRAGMENTS)]
                #[linkme(crate = #codegen::linkme)]
                static __QT_WIDGET_CODEGEN_FRAGMENT: &#codegen::WidgetCodegenFragment =
                    &#codegen::WidgetCodegenFragment {
                        spec_key: #decl::SpecWidgetKey::new(concat!(
                            module_path!(),
                            "::",
                            stringify!(#widget_ident)
                        )),
                        decl: __qt_host_codegen_decl,
                    };
            };
        })
    }
}

fn parse_paint_impl(trait_path: &Path) -> syn::Result<PaintImplConfig> {
    let segment = trait_path.segments.last().ok_or_else(|| {
        syn::Error::new_spanned(trait_path, "#[qt(host)] paint impl requires Paint<Target>")
    })?;
    if segment.ident != "Paint" {
        return Err(syn::Error::new_spanned(
            trait_path,
            "#[qt(host)] paint impl requires Paint<Target>",
        ));
    }

    let syn::PathArguments::AngleBracketed(arguments) = &segment.arguments else {
        return Err(syn::Error::new_spanned(
            trait_path,
            "#[qt(host)] paint impl requires Paint<Target>",
        ));
    };

    let mut args = arguments.args.iter();
    let target_ty = match args.next() {
        Some(syn::GenericArgument::Type(target_ty)) => target_ty.clone(),
        _ => {
            return Err(syn::Error::new_spanned(
                trait_path,
                "#[qt(host)] paint impl requires Paint<Target>",
            ));
        }
    };
    if args.next().is_some() {
        return Err(syn::Error::new_spanned(
            trait_path,
            "#[qt(host)] paint impl accepts exactly one target type",
        ));
    }

    let target_ctor = strip_type_path_generics(&target_ty)?;
    let target_segment = target_ctor.segments.last().ok_or_else(|| {
        syn::Error::new_spanned(
            &target_ty,
            "#[qt(host)] paint impl requires a named target type",
        )
    })?;
    let kind = match target_segment.ident.to_string().as_str() {
        "VelloFrame" | "PaintSceneFrame" => PaintImplKind::Vello,
        "QtPainter" => PaintImplKind::QPainter,
        _ => {
            return Err(syn::Error::new_spanned(
                &target_ty,
                "#[qt(host)] paint impl only supports Paint<PaintSceneFrame<'_>> or Paint<QtPainter<'_>>",
            ));
        }
    };

    Ok(PaintImplConfig {
        target_ty,
        target_ctor,
        kind,
    })
}

fn build_paint_runtime_meta(
    runtime: &proc_macro2::TokenStream,
    self_ty: &Type,
    config: &PaintImplConfig,
) -> proc_macro2::TokenStream {
    let target_ctor = &config.target_ctor;
    match config.kind {
        PaintImplKind::Vello => quote! {{
            unsafe fn __qt_host_paint(
                raw: *mut (),
                device: #runtime::PaintDevice<'_>,
            ) -> #runtime::WidgetResult<()> {
                let widget = unsafe { &mut *(raw.cast::<#self_ty>()) };
                match device {
                    #runtime::PaintDevice::Scene(frame) => {
                        #runtime::Paint::paint(widget, frame);
                        Ok(())
                    }
                    #runtime::PaintDevice::OpaqueHost(_) => Err(
                        #runtime::WidgetError::unsupported_paint_device(device.kind_name())
                    ),
                }
            }

            #runtime::WidgetPaintRuntimeMeta {
                rust_name: "paint",
                invoke: __qt_host_paint,
            }
        }},
        PaintImplKind::QPainter => quote! {{
            unsafe fn __qt_host_paint(
                raw: *mut (),
                device: #runtime::PaintDevice<'_>,
            ) -> #runtime::WidgetResult<()> {
                let widget = unsafe { &mut *(raw.cast::<#self_ty>()) };
                match device {
                    #runtime::PaintDevice::OpaqueHost(host) => {
                        let mut painter = #target_ctor::__qt_from_host(host)?;
                        #runtime::Paint::paint(widget, &mut painter);
                        Ok(())
                    }
                    #runtime::PaintDevice::Scene(_) => Err(
                        #runtime::WidgetError::unsupported_paint_device(device.kind_name())
                    ),
                }
            }

            #runtime::WidgetPaintRuntimeMeta {
                rust_name: "paint",
                invoke: __qt_host_paint,
            }
        }},
    }
}

fn expand_qt_host_trait_decl(
    attr_config: HostTraitDeclConfig,
    mut input: ItemTrait,
) -> syn::Result<proc_macro2::TokenStream> {
    let codegen = widget_core_codegen_path();
    let runtime = widget_core_runtime_path();
    let schema = widget_core_schema_path();
    let vis = input.vis.clone();
    let trait_ident = input.ident.clone();
    let helper_spec_ident = helper_ident_for_trait(&trait_ident, HostDeclHelperKind::Spec);
    let helper_codegen_ident = helper_ident_for_trait(&trait_ident, HostDeclHelperKind::Codegen);
    let helper_runtime_ident = helper_ident_for_trait(&trait_ident, HostDeclHelperKind::Runtime);
    let context = LitStr::new(&trait_ident.to_string(), proc_macro2::Span::call_site());
    let trait_generics = input.generics.clone();
    let (trait_impl_generics, trait_ty_generics, trait_where_clause) =
        trait_generics.split_for_impl();

    let mut overrides = Vec::new();
    let mut event_mounts = Vec::new();
    let mut prop_setters = Vec::new();
    let mut prop_getters = Vec::new();
    let mut prop_specs = Vec::<HostPropRecord>::new();
    let mut spec_events = Vec::new();
    let mut runtime_events = Vec::new();
    let mut runtime_overrides = Vec::new();
    let mut retained_items = Vec::new();
    let used_capabilities = collect_capability_paths(&input.supertraits)?;
    input.supertraits = Punctuated::new();
    let used_spec_refs =
        collect_capability_decl_refs(&used_capabilities, HostDeclHelperKind::Spec)?;
    let used_codegen_refs =
        collect_capability_decl_refs(&used_capabilities, HostDeclHelperKind::Codegen)?;
    let used_runtime_refs =
        collect_capability_decl_refs(&used_capabilities, HostDeclHelperKind::Runtime)?;

    for mut item in std::mem::take(&mut input.items) {
        let TraitItem::Fn(method) = &mut item else {
            return Err(syn::Error::new_spanned(
                item,
                "#[qt(host)] trait declarations only support methods",
            ));
        };

        let has_qt_attrs = method.attrs.iter().any(|attr| attr.path().is_ident("qt"));
        if !has_qt_attrs {
            retained_items.push(item);
            continue;
        }

        let config = parse_host_trait_method_config(&mut method.sig, &method.attrs)?;
        method.attrs.retain(|attr| !attr.path().is_ident("qt"));
        match config {
            HostTraitMethodConfig::Codegen(config) => {
                let default = method.default.as_ref().ok_or_else(|| {
                    syn::Error::new_spanned(
                        &method.sig,
                        "#[qt(host)] trait prop methods require a default qt::cpp! body",
                    )
                })?;
                let Some(body) = qt_cpp_macro_body(default)? else {
                    return Err(syn::Error::new_spanned(
                        default,
                        "#[qt(host)] trait prop methods require a qt::cpp! body",
                    ));
                };
                let rust_name = LitStr::new(&method.sig.ident.to_string(), method.sig.ident.span());
                let extra_includes = config.extra_includes.iter();
                method.default = Some(host_trait_placeholder_body(&method.sig)?);

                push_host_codegen_meta(
                    &codegen,
                    config.kind.clone(),
                    rust_name,
                    extra_includes,
                    body,
                    &mut overrides,
                    &mut event_mounts,
                    &mut prop_setters,
                    &mut prop_getters,
                );
                if let Some(prop) = config.prop.as_ref() {
                    merge_host_prop_record(&mut prop_specs, &config.kind, prop)?;
                }
            }
            HostTraitMethodConfig::Signal(config) => {
                spec_events.push(build_host_event_spec(&schema, &method.sig, &config)?);
                runtime_events.push(build_host_event_runtime_meta(
                    &runtime,
                    &trait_ident,
                    &method.sig,
                    &config,
                )?);
                if method.default.is_none() {
                    method.default = Some(host_trait_noop_body(&method.sig)?);
                } else if let Some(default) = method.default.as_ref() {
                    if qt_cpp_macro_body(default)?.is_some() {
                        return Err(syn::Error::new_spanned(
                            default,
                            "#[qt(host)] signal methods do not accept qt::cpp! bodies",
                        ));
                    }
                }
            }
            HostTraitMethodConfig::Event(config) => {
                let default = method.default.as_ref().ok_or_else(|| {
                    syn::Error::new_spanned(
                        &method.sig,
                        "#[qt(host)] event methods require a default qt::cpp! body",
                    )
                })?;
                let Some(body) = qt_cpp_macro_body(default)? else {
                    return Err(syn::Error::new_spanned(
                        default,
                        "#[qt(host)] event methods require a qt::cpp! body",
                    ));
                };
                let rust_name = LitStr::new(&method.sig.ident.to_string(), method.sig.ident.span());
                let extra_includes = config.extra_includes.iter();
                let event_lower_name =
                    LitStr::new(&config.lower_name, proc_macro2::Span::call_site());
                event_mounts.push(quote! {
                    #codegen::WidgetHostEventMountCodegenMeta {
                        rust_name: #rust_name,
                        event_lower_name: #event_lower_name,
                        lowering: #codegen::HostCodegenLowering {
                            extra_includes: &[
                                #(#extra_includes,)*
                            ],
                            body: #body,
                        },
                    }
                });
                spec_events.push(build_host_event_spec(&schema, &method.sig, &config)?);
                runtime_events.push(build_host_event_runtime_meta(
                    &runtime,
                    &trait_ident,
                    &method.sig,
                    &config,
                )?);
                method.default = Some(host_trait_noop_body(&method.sig)?);
            }
            HostTraitMethodConfig::Override(config) => {
                let default = method.default.as_ref().ok_or_else(|| {
                    syn::Error::new_spanned(
                        &method.sig,
                        "#[qt(host)] override-like methods require a default qt::cpp! body",
                    )
                })?;
                let Some(body) = qt_cpp_macro_body(default)? else {
                    return Err(syn::Error::new_spanned(
                        default,
                        "#[qt(host)] override-like methods require a qt::cpp! body",
                    ));
                };
                overrides.push(build_host_override_codegen_meta(
                    &codegen,
                    &runtime,
                    &method.sig,
                    &config,
                    body,
                )?);
                runtime_overrides.push(build_host_override_runtime_meta(
                    &runtime,
                    &trait_ident,
                    &method.sig,
                )?);
                method.default = None;
            }
        }
        retained_items.push(item);
    }

    input.items = retained_items;
    input.items.push(syn::parse_quote! {
        #[doc(hidden)]
        fn __qt_host_spec_decl() -> &'static #schema::HostCapabilitySpecDecl
        where
            Self: Sized,
        {
            <() as #helper_spec_ident #trait_ty_generics>::decl()
        }
    });
    input.items.push(syn::parse_quote! {
        #[doc(hidden)]
        fn __qt_host_codegen_decl() -> &'static #codegen::HostCapabilityCodegenDecl
        where
            Self: Sized,
        {
            <() as #helper_codegen_ident #trait_ty_generics>::decl()
        }
    });
    input.items.push(syn::parse_quote! {
        #[doc(hidden)]
        fn __qt_host_runtime_decl() -> &'static #runtime::HostBehaviorRuntimeDecl
        where
            Self: Sized,
        {
            <() as #helper_runtime_ident #trait_ty_generics>::decl_for::<Self>()
        }
    });

    let host_tokens = attr_config
        .host
        .as_ref()
        .map(expand_host_meta)
        .map(|host| quote!(Some(#host)))
        .unwrap_or_else(|| quote!(None));
    let prop_specs = prop_specs
        .iter()
        .map(|record| build_host_prop_spec(&schema, record))
        .collect::<syn::Result<Vec<_>>>()?;
    let default_layout =
        resolve_default_layout(attr_config.default_layout, &trait_ident, &trait_generics)?;
    let runtime_widget_bound = if runtime_events.is_empty() && runtime_overrides.is_empty() {
        quote!()
    } else {
        quote!(where Widget: #trait_ident #trait_ty_generics)
    };

    Ok(quote! {
        #input

        #[doc(hidden)]
        #vis trait #helper_spec_ident #trait_generics {
            fn decl() -> &'static #schema::HostCapabilitySpecDecl;
        }

        impl #trait_impl_generics #helper_spec_ident #trait_ty_generics for () #trait_where_clause {
            fn decl() -> &'static #schema::HostCapabilitySpecDecl {
                static DECL: std::sync::OnceLock<&'static #schema::HostCapabilitySpecDecl> =
                    std::sync::OnceLock::new();

                DECL.get_or_init(|| {
                    let own_decl: &'static #schema::HostCapabilitySpecDecl = std::boxed::Box::leak(std::boxed::Box::new(
                        #schema::HostCapabilitySpecDecl {
                            host: #host_tokens,
                            default_layout: #default_layout,
                            props: std::boxed::Box::leak(std::vec![
                                #(#prop_specs,)*
                            ].into_boxed_slice()),
                            events: std::boxed::Box::leak(std::vec![
                                #(#spec_events,)*
                            ].into_boxed_slice()),
                            methods: &#schema::NO_METHODS,
                        }
                    ));
                    let mut decls = std::vec::Vec::new();
                    #(decls.push(#used_spec_refs);)*
                    decls.push(own_decl);
                    #schema::merge_host_spec_decls(#context, decls.as_slice())
                })
            }
        }

        #[doc(hidden)]
        #vis trait #helper_codegen_ident #trait_generics {
            fn decl() -> &'static #codegen::HostCapabilityCodegenDecl;
        }

        impl #trait_impl_generics #helper_codegen_ident #trait_ty_generics for () #trait_where_clause {
            fn decl() -> &'static #codegen::HostCapabilityCodegenDecl {
                static DECL: std::sync::OnceLock<&'static #codegen::HostCapabilityCodegenDecl> =
                    std::sync::OnceLock::new();

                DECL.get_or_init(|| {
                    let own_decl: &'static #codegen::HostCapabilityCodegenDecl = std::boxed::Box::leak(std::boxed::Box::new(
                        #codegen::HostCapabilityCodegenDecl {
                            host_overrides: std::boxed::Box::leak(std::boxed::Box::new(
                                #codegen::WidgetHostOverrideCodegenSet {
                                    overrides: std::boxed::Box::leak(std::vec![
                                        #(#overrides,)*
                                    ].into_boxed_slice()),
                                }
                            )),
                            host_event_mounts: std::boxed::Box::leak(std::boxed::Box::new(
                                #codegen::WidgetHostEventMountCodegenSet {
                                    mounts: std::boxed::Box::leak(std::vec![
                                        #(#event_mounts,)*
                                    ].into_boxed_slice()),
                                }
                            )),
                            host_prop_setters: std::boxed::Box::leak(std::boxed::Box::new(
                                #codegen::WidgetHostPropSetterCodegenSet {
                                    setters: std::boxed::Box::leak(std::vec![
                                        #(#prop_setters,)*
                                    ].into_boxed_slice()),
                                }
                            )),
                            host_prop_getters: std::boxed::Box::leak(std::boxed::Box::new(
                                #codegen::WidgetHostPropGetterCodegenSet {
                                    getters: std::boxed::Box::leak(std::vec![
                                        #(#prop_getters,)*
                                    ].into_boxed_slice()),
                                }
                            )),
                        }
                    ));
                    let mut decls = std::vec::Vec::new();
                    #(decls.push(#used_codegen_refs);)*
                    decls.push(own_decl);
                    #codegen::merge_host_codegen_decls(#context, decls.as_slice())
                })
            }
        }

        #[doc(hidden)]
        #vis trait #helper_runtime_ident #trait_generics {
            fn decl_for<Widget>() -> &'static #runtime::HostBehaviorRuntimeDecl
            #runtime_widget_bound;
        }

        impl #trait_impl_generics #helper_runtime_ident #trait_ty_generics for () #trait_where_clause {
            fn decl_for<Widget>() -> &'static #runtime::HostBehaviorRuntimeDecl
            #runtime_widget_bound
            {
                static DECL: std::sync::OnceLock<&'static #runtime::HostBehaviorRuntimeDecl> =
                    std::sync::OnceLock::new();

                DECL.get_or_init(|| {
                    let own_decl: &'static #runtime::HostBehaviorRuntimeDecl =
                        std::boxed::Box::leak(std::boxed::Box::new(
                            #runtime::HostBehaviorRuntimeDecl {
                                host_events: std::boxed::Box::leak(std::boxed::Box::new(
                                    #runtime::WidgetHostEventRuntimeSet {
                                        events: std::boxed::Box::leak(std::vec![
                                            #(#runtime_events,)*
                                        ].into_boxed_slice()),
                                    }
                                )),
                                host_overrides: std::boxed::Box::leak(std::boxed::Box::new(
                                    #runtime::WidgetHostOverrideRuntimeSet {
                                        overrides: std::boxed::Box::leak(std::vec![
                                            #(#runtime_overrides,)*
                                        ].into_boxed_slice()),
                                    }
                                )),
                                paint: core::option::Option::None,
                            }
                        ));
                    let mut decls = std::vec::Vec::new();
                    #(decls.push(#used_runtime_refs);)*
                    decls.push(own_decl);
                    #runtime::merge_host_behavior_runtime_decls(#context, decls.as_slice())
                })
            }
        }
    })
}

fn infer_default_layout(
    trait_ident: &Ident,
    generics: &syn::Generics,
) -> syn::Result<Option<proc_macro2::TokenStream>> {
    if trait_ident != "Layout" {
        return Ok(None);
    }

    let mut layout_kind_binding = None::<(Ident, Path)>;

    let mut record_binding = |ident: &Ident, path: &Path| -> syn::Result<()> {
        if path
            .segments
            .last()
            .map(|segment| segment.ident != "LayoutKind")
            != Some(false)
        {
            return Ok(());
        }
        if let Some((existing_ident, _)) = &layout_kind_binding {
            if existing_ident != ident {
                return Err(syn::Error::new_spanned(
                    trait_ident,
                    "Layout trait must have exactly one type parameter bounded by LayoutKind",
                ));
            }
            return Ok(());
        }
        layout_kind_binding = Some((ident.clone(), path.clone()));
        Ok(())
    };

    for param in &generics.params {
        let GenericParam::Type(param) = param else {
            continue;
        };
        for bound in &param.bounds {
            let TypeParamBound::Trait(bound) = bound else {
                continue;
            };
            record_binding(&param.ident, &bound.path)?;
        }
    }

    if let Some(where_clause) = &generics.where_clause {
        for predicate in &where_clause.predicates {
            let WherePredicate::Type(predicate) = predicate else {
                continue;
            };
            let Type::Path(type_path) = &predicate.bounded_ty else {
                continue;
            };
            let Some(param_ident) = type_path.path.get_ident() else {
                continue;
            };
            for bound in &predicate.bounds {
                let TypeParamBound::Trait(bound) = bound else {
                    continue;
                };
                record_binding(param_ident, &bound.path)?;
            }
        }
    }

    let Some((ident, bound_path)) = layout_kind_binding else {
        return Err(syn::Error::new_spanned(
            trait_ident,
            "Layout trait must declare a type parameter bounded by LayoutKind",
        ));
    };

    Ok(Some(quote!(<#ident as #bound_path>::LAYOUT)))
}

fn resolve_default_layout(
    attr_default_layout: Option<Expr>,
    trait_ident: &Ident,
    generics: &syn::Generics,
) -> syn::Result<proc_macro2::TokenStream> {
    let inferred_default_layout = infer_default_layout(trait_ident, generics)?;
    Ok(match (attr_default_layout, inferred_default_layout) {
        (Some(_), Some(_)) => {
            return Err(syn::Error::new_spanned(
                trait_ident,
                "Layout<K> derives its host layout fact from K: LayoutKind; remove redundant layout = ...",
            ));
        }
        (Some(layout), None) => quote!(Some(#layout)),
        (None, Some(layout)) => quote!(Some(#layout)),
        (None, None) => quote!(None),
    })
}

fn push_host_codegen_meta<'a>(
    codegen: &proc_macro2::TokenStream,
    kind: HostCodegenMethodKind,
    rust_name: LitStr,
    extra_includes: impl Iterator<Item = &'a LitStr>,
    body: LitStr,
    _overrides: &mut Vec<proc_macro2::TokenStream>,
    _event_mounts: &mut Vec<proc_macro2::TokenStream>,
    prop_setters: &mut Vec<proc_macro2::TokenStream>,
    prop_getters: &mut Vec<proc_macro2::TokenStream>,
) {
    match kind {
        HostCodegenMethodKind::PropSetter {
            prop_lower_name,
            arg_name,
            value_type,
        } => {
            let prop_lower_name = LitStr::new(&prop_lower_name, proc_macro2::Span::call_site());
            let arg_name = LitStr::new(&arg_name, proc_macro2::Span::call_site());

            prop_setters.push(quote! {
                #codegen::WidgetHostPropSetterCodegenMeta {
                    rust_name: #rust_name,
                    prop_lower_name: #prop_lower_name,
                    arg_name: #arg_name,
                    value_type: #value_type,
                    lowering: #codegen::HostCodegenLowering {
                        extra_includes: &[
                            #(#extra_includes,)*
                        ],
                        body: #body,
                    },
                }
            });
        }
        HostCodegenMethodKind::PropGetter {
            prop_lower_name,
            value_type,
        } => {
            let prop_lower_name = LitStr::new(&prop_lower_name, proc_macro2::Span::call_site());

            prop_getters.push(quote! {
                #codegen::WidgetHostPropGetterCodegenMeta {
                    rust_name: #rust_name,
                    prop_lower_name: #prop_lower_name,
                    value_type: #value_type,
                    lowering: #codegen::HostCodegenLowering {
                        extra_includes: &[
                            #(#extra_includes,)*
                        ],
                        body: #body,
                    },
                }
            });
        }
    }
}

fn emit_inherent_host_spec_fragment(
    schema: &proc_macro2::TokenStream,
    decl: &proc_macro2::TokenStream,
    widget_ident: &Ident,
    prop_specs: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote! {
        const _: () = {
            use #schema::linkme::distributed_slice;

            fn __qt_host_spec_decl() -> &'static #schema::HostCapabilitySpecDecl {
                static DECL: std::sync::OnceLock<&'static #schema::HostCapabilitySpecDecl> =
                    std::sync::OnceLock::new();

                DECL.get_or_init(|| {
                    std::boxed::Box::leak(std::boxed::Box::new(
                        #schema::HostCapabilitySpecDecl {
                            host: None,
                            default_layout: None,
                            props: std::boxed::Box::leak(std::vec![
                                #(#prop_specs,)*
                            ].into_boxed_slice()),
                            events: &[],
                            methods: &#schema::NO_METHODS,
                        }
                    ))
                })
            }

            #[distributed_slice(#schema::QT_WIDGET_HOST_SPEC_FRAGMENTS)]
            #[linkme(crate = #schema::linkme)]
            static __QT_WIDGET_HOST_SPEC_FRAGMENT: &#schema::WidgetHostSpecFragment =
                &#schema::WidgetHostSpecFragment {
                    spec_key: #decl::SpecWidgetKey::new(concat!(
                        module_path!(),
                        "::",
                        stringify!(#widget_ident)
                    )),
                    decl: __qt_host_spec_decl,
                };
        };
    }
}

fn merge_host_prop_record(
    records: &mut Vec<HostPropRecord>,
    kind: &HostCodegenMethodKind,
    prop: &HostPropSpecConfig,
) -> syn::Result<()> {
    let value_type = match kind {
        HostCodegenMethodKind::PropSetter { value_type, .. }
        | HostCodegenMethodKind::PropGetter { value_type, .. } => value_type.clone(),
    };
    let value_type_key = quote!(#value_type).to_string();

    if let Some(existing) = records
        .iter_mut()
        .find(|record| record.spec.js_name == prop.js_name)
    {
        if existing.value_type_key != value_type_key {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!(
                    "widget prop {} mixes incompatible Rust value types {} and {}",
                    prop.js_name, existing.value_type_key, value_type_key
                ),
            ));
        }
        if !matches!(
            (existing.spec.behavior, prop.behavior),
            (HostPropBehavior::State, HostPropBehavior::State)
                | (HostPropBehavior::Const, HostPropBehavior::Const)
                | (HostPropBehavior::Command, HostPropBehavior::Command)
        ) {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("widget prop {} mixes incompatible behaviors", prop.js_name),
            ));
        }
        if existing.spec.default.is_none() {
            existing.spec.default = prop.default.clone();
        } else if prop.default.is_some() {
            return Err(syn::Error::new(
                proc_macro2::Span::call_site(),
                format!("widget prop {} declares duplicate defaults", prop.js_name),
            ));
        }
        return Ok(());
    }

    records.push(HostPropRecord {
        spec: prop.clone(),
        value_type,
        value_type_key,
    });
    Ok(())
}

fn build_host_prop_spec(
    schema: &proc_macro2::TokenStream,
    record: &HostPropRecord,
) -> syn::Result<proc_macro2::TokenStream> {
    let prop = &record.spec;
    let rust_name = LitStr::new(&prop.rust_name, proc_macro2::Span::call_site());
    let js_name = LitStr::new(&prop.js_name, proc_macro2::Span::call_site());
    let lower_name = internal_host_prop_lower_name(&prop.js_name);
    let lower_name = LitStr::new(&lower_name, proc_macro2::Span::call_site());
    let value_type = &record.value_type;
    let behavior = match prop.behavior {
        HostPropBehavior::State => quote!(#schema::PropBehavior::State),
        HostPropBehavior::Const => quote!(#schema::PropBehavior::Const),
        HostPropBehavior::Command => quote!(#schema::PropBehavior::Command),
    };
    let read_lowering = match prop.behavior {
        HostPropBehavior::State => {
            quote!(Some(#schema::PropLowering::Custom(#lower_name)))
        }
        HostPropBehavior::Const | HostPropBehavior::Command => quote!(None),
    };
    let default = host_prop_default_value_tokens(schema, prop.default.as_ref())?;

    Ok(quote! {
        #schema::SpecLeafProp {
            rust_name: #rust_name,
            js_name: #js_name,
            value_type: #value_type,
            optional: true,
            lowering: #schema::PropLowering::Custom(#lower_name),
            read_lowering: #read_lowering,
            behavior: #behavior,
            exported: true,
            default: #default,
        }
    })
}

fn host_prop_default_value_tokens(
    schema: &proc_macro2::TokenStream,
    default: Option<&Expr>,
) -> syn::Result<proc_macro2::TokenStream> {
    let Some(default) = default else {
        return Ok(quote!(#schema::SpecPropDefaultValue::None));
    };

    match default {
        Expr::Lit(expr_lit) => match &expr_lit.lit {
            syn::Lit::Bool(value) => Ok(quote!(#schema::SpecPropDefaultValue::Bool(#value))),
            syn::Lit::Int(value) => {
                let value = value.base10_parse::<i32>()?;
                Ok(quote!(#schema::SpecPropDefaultValue::I32(#value)))
            }
            syn::Lit::Float(value) => {
                let value = value.base10_parse::<f64>()?;
                Ok(quote!(#schema::SpecPropDefaultValue::F64(#value)))
            }
            syn::Lit::Str(value) => Ok(quote!(#schema::SpecPropDefaultValue::String(#value))),
            other => Err(syn::Error::new_spanned(
                other,
                "unsupported widget prop default literal",
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
                    "unsupported widget prop default expression",
                ));
            }
            host_prop_default_value_tokens(schema, call.args.first())
        }
        other => Err(syn::Error::new_spanned(
            other,
            "unsupported widget prop default expression",
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

fn collect_capability_decl_refs(
    uses: &[Path],
    kind: HostDeclHelperKind,
) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    uses.iter()
        .cloned()
        .map(|path| {
            let helper_path = host_decl_helper_path(&path, kind)?;
            Ok(match kind {
                HostDeclHelperKind::Runtime => quote!(<() as #helper_path>::decl_for::<Widget>()),
                HostDeclHelperKind::Spec | HostDeclHelperKind::Codegen => {
                    quote!(<() as #helper_path>::decl())
                }
            })
        })
        .collect()
}

fn host_trait_placeholder_body(sig: &syn::Signature) -> syn::Result<syn::Block> {
    let suppress_unused = sig.inputs.iter().filter_map(|arg| match arg {
        FnArg::Receiver(_) => None,
        FnArg::Typed(arg) => match arg.pat.as_ref() {
            syn::Pat::Ident(pat_ident) => {
                let ident = &pat_ident.ident;
                Some(quote!(let _ = &#ident;))
            }
            _ => None,
        },
    });

    syn::parse2(quote!({
        #(#suppress_unused;)*
        panic!("qt host lowering-only method")
    }))
}

fn host_trait_noop_body(sig: &syn::Signature) -> syn::Result<syn::Block> {
    let suppress_unused = sig.inputs.iter().filter_map(|arg| match arg {
        FnArg::Receiver(_) => None,
        FnArg::Typed(arg) => match arg.pat.as_ref() {
            syn::Pat::Ident(pat_ident) => {
                let ident = &pat_ident.ident;
                Some(quote!(let _ = &#ident;))
            }
            _ => None,
        },
    });
    let returns_result = host_behavior_return_is_result(&sig.output)?;

    if returns_result {
        syn::parse2(quote!({
            #(#suppress_unused;)*
            Ok(())
        }))
    } else {
        syn::parse2(quote!({
            #(#suppress_unused;)*
        }))
    }
}

fn internal_host_prop_lower_name(js_name: &str) -> String {
    js_name.to_owned()
}

fn validate_host_event_method(sig: &syn::Signature) -> syn::Result<()> {
    let Some(first_arg) = sig.inputs.first() else {
        return Err(syn::Error::new_spanned(
            sig,
            "#[qt(host)] event methods require &self or &mut self",
        ));
    };
    let FnArg::Receiver(receiver) = first_arg else {
        return Err(syn::Error::new_spanned(
            first_arg,
            "#[qt(host)] event methods require a self receiver",
        ));
    };
    if receiver.reference.is_none() || receiver.colon_token.is_some() {
        return Err(syn::Error::new_spanned(
            receiver,
            "#[qt(host)] event methods require &self or &mut self",
        ));
    }

    match &sig.output {
        ReturnType::Default => Ok(()),
        ReturnType::Type(_, ty) if is_unit_type(ty) => Ok(()),
        ReturnType::Type(_, ty)
            if unwrap_result_type(ty)?.is_some_and(
                |inner| matches!(inner, Type::Tuple(tuple) if tuple.elems.is_empty()),
            ) =>
        {
            Ok(())
        }
        _ => Err(syn::Error::new_spanned(
            &sig.output,
            "#[qt(host)] event methods must return () or WidgetResult<()>",
        )),
    }
}

fn validate_host_override_target_method(sig: &syn::Signature) -> syn::Result<()> {
    validate_host_event_method(sig)?;
    if sig.inputs.len() > 2 {
        return Err(syn::Error::new_spanned(
            sig,
            "#[qt(host)] override-like methods currently support at most one explicit argument",
        ));
    }

    if let Some(FnArg::Typed(arg)) = sig.inputs.iter().nth(1) {
        if option_inner_type(&arg.ty).is_some() {
            return Err(syn::Error::new_spanned(
                &arg.ty,
                "#[qt(host)] override-like methods do not support Option<_> opaque arguments",
            ));
        }
    }

    Ok(())
}

fn build_host_event_spec(
    schema: &proc_macro2::TokenStream,
    sig: &syn::Signature,
    config: &HostBehaviorEventConfig,
) -> syn::Result<proc_macro2::TokenStream> {
    let rust_name = LitStr::new(&sig.ident.to_string(), sig.ident.span());
    let exports = config
        .exports
        .iter()
        .map(|export| LitStr::new(export, proc_macro2::Span::call_site()))
        .collect::<Vec<_>>();
    let label = LitStr::new(&config.label, proc_macro2::Span::call_site());
    let args = host_behavior_args(sig)?;

    let (payload_kind, payload_type, payload_fields) = match args.as_slice() {
        [] => (
            quote!(#schema::EventPayloadKind::Unit),
            quote!(None),
            quote!(&[]),
        ),
        [(_, ty, _)] => (
            quote!(#schema::EventPayloadKind::Scalar),
            quote!(Some(<#ty as #schema::QtType>::INFO)),
            quote!(&[]),
        ),
        fields => {
            let fields = fields.iter().map(|(ident, ty, js_name)| {
                let rust_name = LitStr::new(&ident.to_string(), ident.span());
                let js_name = LitStr::new(js_name, ident.span());
                quote! {
                    #schema::EventFieldMeta {
                        rust_name: #rust_name,
                        js_name: #js_name,
                        value_type: <#ty as #schema::QtType>::INFO,
                    }
                }
            });
            (
                quote!(#schema::EventPayloadKind::Object),
                quote!(None),
                quote!(&[
                    #(#fields,)*
                ]),
            )
        }
    };
    let lowering = if let Some(qt_signal_name) = &config.qt_signal_name {
        let qt_signal_name = LitStr::new(qt_signal_name, proc_macro2::Span::call_site());
        quote!(#schema::EventLowering::QtSignal(#qt_signal_name))
    } else {
        let lower_name = LitStr::new(&config.lower_name, proc_macro2::Span::call_site());
        quote!(#schema::EventLowering::Custom(#lower_name))
    };
    let echoes = config.echoes.iter().map(|echo| {
        let prop_js_name = LitStr::new(&echo.prop_js_name, proc_macro2::Span::call_site());
        let value_path = LitStr::new(&echo.value_path, proc_macro2::Span::call_site());
        quote! {
            #schema::EventEchoMeta {
                prop_js_name: #prop_js_name,
                value_path: #value_path,
            }
        }
    });

    Ok(quote! {
        #schema::SpecEventMeta {
            index: 0,
            rust_name: #rust_name,
            exports: &[
                #(#exports,)*
            ],
            payload_kind: #payload_kind,
            payload_type: #payload_type,
            payload_fields: #payload_fields,
            echoes: &[
                #(#echoes,)*
            ],
            label: #label,
            lowering: #lowering,
        }
    })
}

fn host_behavior_return_is_result(output: &ReturnType) -> syn::Result<bool> {
    match output {
        ReturnType::Default => Ok(false),
        ReturnType::Type(_, ty) => Ok(unwrap_result_type(ty)?.is_some()),
    }
}

fn build_host_event_runtime_meta(
    runtime: &proc_macro2::TokenStream,
    trait_ident: &Ident,
    sig: &syn::Signature,
    _config: &HostBehaviorEventConfig,
) -> syn::Result<proc_macro2::TokenStream> {
    let rust_name = LitStr::new(&sig.ident.to_string(), sig.ident.span());
    let helper_name = format_ident!("__qt_host_event_{}", sig.ident, span = sig.ident.span());
    let method_ident = &sig.ident;
    let args = host_behavior_args(sig)?;
    let returns_result = host_behavior_return_is_result(&sig.output)?;

    let conversions = args
        .iter()
        .enumerate()
        .map(|(index, (ident, ty, _))| {
            quote! {
                let #ident = <#ty as #runtime::TryFromQt>::try_from_qt(
                    args
                        .get(#index)
                        .cloned()
                        .ok_or_else(|| #runtime::WidgetError::new(format!(
                            "host event {} is missing argument {}",
                            #rust_name,
                            #index
                        )))?
                )?;
            }
        })
        .collect::<Vec<_>>();
    let arg_idents = args.iter().map(|(ident, _, _)| ident).collect::<Vec<_>>();
    let invoke = if returns_result {
        quote!(<T as #trait_ident>::#method_ident(widget, #(#arg_idents),*))
    } else {
        quote!({
            <T as #trait_ident>::#method_ident(widget, #(#arg_idents),*);
            Ok(())
        })
    };

    Ok(quote! {{
        unsafe fn #helper_name<T: #trait_ident>(
            raw: *mut (),
            args: &[#runtime::QtValue],
        ) -> #runtime::WidgetResult<()> {
            let widget = &mut *(raw.cast::<T>());
            #(#conversions)*
            #invoke
        }

        #runtime::WidgetHostEventRuntimeMeta {
            rust_name: #rust_name,
            invoke: #helper_name::<Widget>,
        }
    }})
}

fn strip_type_path_generics(ty: &Type) -> syn::Result<Path> {
    let Type::Path(type_path) = ty else {
        return Err(syn::Error::new_spanned(
            ty,
            "opaque arguments require a named type",
        ));
    };

    let mut path = type_path.path.clone();
    let Some(last) = path.segments.last_mut() else {
        return Err(syn::Error::new_spanned(
            ty,
            "opaque arguments require a named type",
        ));
    };
    last.arguments = syn::PathArguments::None;
    Ok(path)
}

fn build_host_override_codegen_meta(
    codegen: &proc_macro2::TokenStream,
    runtime: &proc_macro2::TokenStream,
    sig: &syn::Signature,
    config: &HostBehaviorOverrideConfig,
    body: LitStr,
) -> syn::Result<proc_macro2::TokenStream> {
    let rust_name = LitStr::new(&sig.ident.to_string(), sig.ident.span());
    let target_name = LitStr::new(&sig.ident.to_string(), sig.ident.span());
    let extra_includes = config.extra_includes.iter();
    let signature = LitStr::new(&config.signature, proc_macro2::Span::call_site());
    let opaque_ty = sig
        .inputs
        .iter()
        .nth(1)
        .and_then(|arg| match arg {
            FnArg::Typed(arg) => Some(arg.ty.as_ref()),
            _ => None,
        })
        .ok_or_else(|| {
            syn::Error::new_spanned(
                sig,
                "#[qt(host)] override-like methods currently require one opaque argument",
            )
        })?;
    let opaque_ctor = strip_type_path_generics(opaque_ty)?;

    Ok(quote! {
        #codegen::WidgetHostOverrideCodegenMeta {
            rust_name: #rust_name,
            target_name: #target_name,
            opaque: <#opaque_ty as #runtime::QtOpaqueFacade>::INFO,
            bridge_fn: <#opaque_ctor as #codegen::QtOpaqueCodegenBridge>::HOOK_BRIDGE_FN,
            signature: #signature,
            lowering: #codegen::HostCodegenLowering {
                extra_includes: &[
                    #(#extra_includes,)*
                ],
                body: #body,
            },
        }
    })
}

fn build_host_override_runtime_meta(
    runtime: &proc_macro2::TokenStream,
    trait_ident: &Ident,
    sig: &syn::Signature,
) -> syn::Result<proc_macro2::TokenStream> {
    let rust_name = LitStr::new(&sig.ident.to_string(), sig.ident.span());
    let helper_name = format_ident!("__qt_host_override_{}", sig.ident, span = sig.ident.span());
    let method_ident = &sig.ident;
    let returns_result = host_behavior_return_is_result(&sig.output)?;
    let opaque_ty = sig
        .inputs
        .iter()
        .nth(1)
        .and_then(|arg| match arg {
            FnArg::Typed(arg) => Some(arg.ty.as_ref()),
            _ => None,
        })
        .ok_or_else(|| {
            syn::Error::new_spanned(
                sig,
                "#[qt(host)] override-like methods currently require one opaque argument",
            )
        })?;
    let opaque_ctor = strip_type_path_generics(opaque_ty)?;
    let invoke = if returns_result {
        quote!(<T as #trait_ident>::#method_ident(widget, opaque))
    } else {
        quote!({
            <T as #trait_ident>::#method_ident(widget, opaque);
            Ok(())
        })
    };

    Ok(quote! {{
        unsafe fn #helper_name<T: #trait_ident>(
            raw: *mut (),
            host: &mut dyn #runtime::QtOpaqueHostMutDyn,
        ) -> #runtime::WidgetResult<()> {
            let widget = &mut *(raw.cast::<T>());
            let opaque = #opaque_ctor::__qt_from_host(host)?;
            #invoke
        }

        #runtime::WidgetHostOverrideRuntimeMeta {
            rust_name: #rust_name,
            invoke: #helper_name::<Widget>,
        }
    }})
}

fn widget_self_ident(self_ty: &Type) -> syn::Result<Ident> {
    let Type::Path(type_path) = self_ty else {
        return Err(syn::Error::new_spanned(
            self_ty,
            "#[qt(host)] requires a named widget type",
        ));
    };
    let Some(last) = type_path.path.segments.last() else {
        return Err(syn::Error::new_spanned(
            self_ty,
            "#[qt(host)] requires a named widget type",
        ));
    };
    Ok(last.ident.clone())
}

fn expand_host_meta(config: &HostConfig) -> proc_macro2::TokenStream {
    let schema = widget_core_schema_path();
    let class = &config.class;
    let include = &config.include;
    let top_level = config.top_level;
    let factory = config
        .factory
        .as_ref()
        .map_or_else(|| quote!(None), |factory| quote!(Some(#factory)));

    quote! {
        #schema::HostMeta {
            class: #class,
            include: #include,
            factory: #factory,
            top_level: #top_level,
        }
    }
}

fn validate_host_prop_setter_method(
    sig: &syn::Signature,
) -> syn::Result<(String, proc_macro2::TokenStream)> {
    let schema = widget_core_schema_path();
    let Some(first_arg) = sig.inputs.first() else {
        return Err(syn::Error::new_spanned(
            sig,
            "#[qt(host)] prop setters require &mut self plus one value argument",
        ));
    };
    let FnArg::Receiver(receiver) = first_arg else {
        return Err(syn::Error::new_spanned(
            first_arg,
            "#[qt(host)] prop setters require &mut self",
        ));
    };
    if receiver.reference.is_none() || receiver.mutability.is_none() {
        return Err(syn::Error::new_spanned(
            receiver,
            "#[qt(host)] prop setters require &mut self",
        ));
    }
    if sig.inputs.len() != 2 {
        return Err(syn::Error::new_spanned(
            sig,
            "#[qt(host)] prop setters require exactly one value argument",
        ));
    }
    match &sig.output {
        ReturnType::Default => {}
        ReturnType::Type(_, ty) if is_unit_type(ty) => {}
        _ => {
            return Err(syn::Error::new_spanned(
                &sig.output,
                "#[qt(host)] prop setters must return unit",
            ));
        }
    }

    let FnArg::Typed(arg) = sig.inputs.iter().nth(1).expect("value arg") else {
        unreachable!();
    };
    let syn::Pat::Ident(pat_ident) = arg.pat.as_ref() else {
        return Err(syn::Error::new_spanned(
            &arg.pat,
            "#[qt(host)] prop setters require a simple value identifier",
        ));
    };
    if option_inner_type(&arg.ty).is_some() {
        return Err(syn::Error::new_spanned(
            &arg.ty,
            "#[qt(host)] prop setters require a concrete value type, not Option<_>",
        ));
    }
    if unwrap_result_type(&arg.ty)?.is_some() {
        return Err(syn::Error::new_spanned(
            &arg.ty,
            "#[qt(host)] prop setters do not accept Result<_> arguments",
        ));
    }
    let arg_ty = &arg.ty;

    Ok((
        pat_ident.ident.to_string(),
        quote!(<#arg_ty as #schema::QtType>::INFO),
    ))
}

fn validate_host_prop_getter_method(sig: &syn::Signature) -> syn::Result<proc_macro2::TokenStream> {
    let schema = widget_core_schema_path();
    let Some(first_arg) = sig.inputs.first() else {
        return Err(syn::Error::new_spanned(
            sig,
            "#[qt(host)] prop getters require &self",
        ));
    };
    let FnArg::Receiver(receiver) = first_arg else {
        return Err(syn::Error::new_spanned(
            first_arg,
            "#[qt(host)] prop getters require &self",
        ));
    };
    if receiver.reference.is_none() || receiver.mutability.is_some() {
        return Err(syn::Error::new_spanned(
            receiver,
            "#[qt(host)] prop getters require &self",
        ));
    }
    if sig.inputs.len() != 1 {
        return Err(syn::Error::new_spanned(
            sig,
            "#[qt(host)] prop getters support no explicit arguments",
        ));
    }

    let ReturnType::Type(_, ty) = &sig.output else {
        return Err(syn::Error::new_spanned(
            &sig.output,
            "#[qt(host)] prop getters must return a value",
        ));
    };
    if is_unit_type(ty) {
        return Err(syn::Error::new_spanned(
            &sig.output,
            "#[qt(host)] prop getters must return a value",
        ));
    }
    if option_inner_type(ty).is_some() {
        return Err(syn::Error::new_spanned(
            ty,
            "#[qt(host)] prop getters require a concrete value type, not Option<_>",
        ));
    }

    Ok(quote!(<#ty as #schema::QtType>::INFO))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_notify_with_qt_signal_as_signal() {
        let mut method: syn::TraitItemFn = syn::parse_quote! {
            #[qt(notify = widget::clicked, qt_signal = "clicked", export = "onClicked")]
            fn clicked(&mut self);
        };

        let config =
            parse_host_trait_method_config(&mut method.sig, &method.attrs).expect("notify config");

        match config {
            HostTraitMethodConfig::Signal(config) => {
                assert_eq!(config.label, "widget::clicked");
                assert_eq!(config.qt_signal_name.as_deref(), Some("clicked"));
                assert_eq!(config.exports, vec!["onClicked"]);
            }
            other => panic!(
                "expected signal config, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn parse_notify_without_qt_signal_as_event() {
        let mut method: syn::TraitItemFn = syn::parse_quote! {
            #[qt(notify = focus::focus_in, export = "onFocusIn")]
            fn focus_in(&mut self);
        };

        let config =
            parse_host_trait_method_config(&mut method.sig, &method.attrs).expect("notify config");

        match config {
            HostTraitMethodConfig::Event(config) => {
                assert_eq!(config.label, "focus::focus_in");
                assert!(config.qt_signal_name.is_none());
                assert_eq!(config.exports, vec!["onFocusIn"]);
            }
            other => panic!(
                "expected event config, got {:?}",
                std::mem::discriminant(&other)
            ),
        }
    }

    #[test]
    fn trait_export_is_notify_only() {
        let mut method: syn::TraitItemFn = syn::parse_quote! {
            #[qt(prop = width, export = "onWidth", setter)]
            fn set_width(&mut self, value: i32);
        };

        let error = match parse_host_trait_method_config(&mut method.sig, &method.attrs) {
            Ok(_) => panic!("must reject"),
            Err(error) => error,
        };

        assert!(
            error
                .to_string()
                .contains("is only supported on #[qt(notify = ...)] methods")
        );
    }

    #[test]
    fn notify_requires_path_label() {
        let mut method: syn::TraitItemFn = syn::parse_quote! {
            #[qt(notify = widget(widget::clicked))]
            fn clicked(&mut self);
        };

        let error = parse_host_trait_method_config(&mut method.sig, &method.attrs)
            .err()
            .expect("host(...) notify path must be rejected");

        assert!(error.to_string().contains("notify label expects path"));
    }

    #[test]
    fn duplicate_notify_export_targets_are_rejected() {
        let mut method: syn::TraitItemFn = syn::parse_quote! {
            #[qt(notify = widget::clicked, export = "onClicked", export = "onClicked")]
            fn clicked(&mut self);
        };

        let error = parse_host_trait_method_config(&mut method.sig, &method.attrs)
            .err()
            .expect("duplicate export target must be rejected");

        assert!(
            error
                .to_string()
                .contains("duplicate #[qt(export = ...)] target onClicked")
        );
    }

    #[test]
    fn duplicate_notify_echo_targets_are_rejected() {
        let mut method: syn::TraitItemFn = syn::parse_quote! {
            #[qt(notify = widget::changed, export = "onChanged")]
            fn changed(
                &mut self,
                #[qt(echo(prop = text), echo(prop = text))] value: String,
            );
        };

        let error = parse_host_trait_method_config(&mut method.sig, &method.attrs)
            .err()
            .expect("duplicate echo target must be rejected");

        assert!(
            error
                .to_string()
                .contains("duplicate #[qt(echo(...))] target text")
        );
    }

    #[test]
    fn echo_requires_notify() {
        let mut method: syn::TraitItemFn = syn::parse_quote! {
            fn changed(&mut self, #[qt(echo)] value: String);
        };

        let error = parse_host_trait_method_config(&mut method.sig, &method.attrs)
            .err()
            .expect("echo without notify must be rejected");

        assert!(
            error
                .to_string()
                .contains("is only supported on #[qt(notify = ...)] methods")
        );
    }

    #[test]
    fn notify_rejects_prop_metadata_mix() {
        let mut method: syn::TraitItemFn = syn::parse_quote! {
            #[qt(notify = widget::clicked, prop = clicked, setter)]
            fn clicked(&mut self);
        };

        let error = parse_host_trait_method_config(&mut method.sig, &method.attrs)
            .err()
            .expect("notify mixed with prop metadata must be rejected");

        assert!(
            error
                .to_string()
                .contains("do not accept prop metadata, setter/getter, or signature")
        );
    }

    #[test]
    fn layout_trait_derives_default_layout_from_kind_bound() {
        let input: syn::ItemTrait = syn::parse_quote! {
            pub trait Layout<K: LayoutKind> {}
        };

        let default_layout =
            resolve_default_layout(None, &input.ident, &input.generics).expect("layout fact");

        assert_eq!(
            default_layout.to_string(),
            "Some (< K as LayoutKind > :: LAYOUT)"
        );
    }

    #[test]
    fn layout_trait_rejects_redundant_layout_attr() {
        let input: syn::ItemTrait = syn::parse_quote! {
            pub trait Layout<K: LayoutKind> {}
        };

        let error = resolve_default_layout(
            Some(syn::parse_quote!(<K as LayoutKind>::LAYOUT)),
            &input.ident,
            &input.generics,
        )
        .err()
        .expect("redundant layout attr must be rejected");

        assert!(
            error
                .to_string()
                .contains("derives its host layout fact from K: LayoutKind")
        );
    }
}
