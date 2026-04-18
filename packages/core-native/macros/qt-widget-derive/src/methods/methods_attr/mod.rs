mod model;
mod parse;

use self::model::{HostMethodReceiverKind, MethodLoweringConfig, OpaqueCodegenLoweringConfig};
use self::parse::{parse_method_config, take_item_level_host_attr};
use super::shared::{is_unit_type, method_return_type, qt_cpp_macro_body, unwrap_result_type};
use crate::common::{
    option_inner_type, snake_to_lower_camel, widget_core_codegen_path, widget_core_decl_path,
    widget_core_runtime_path, widget_core_schema_path,
};
use crate::prop_methods::{self, QtMethodImplKind};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::{FnArg, Ident, Item, ItemImpl, ItemTrait, LitStr, Pat, ReturnType, TraitItem, Type};

pub(crate) fn expand_qt_methods_attr(input: Item) -> syn::Result<proc_macro2::TokenStream> {
    match input {
        Item::Impl(mut input) if input.trait_.is_some() => {
            if let Some(host_attr) = take_item_level_host_attr(&mut input.attrs)? {
                return super::host_attr::expand_qt_host_attr(host_attr, Item::Impl(input));
            }
            expand_qt_methods_attach_impl(input)
        }
        Item::Impl(mut input) => {
            if let Some(host_attr) = take_item_level_host_attr(&mut input.attrs)? {
                return super::host_attr::expand_qt_host_attr(host_attr, Item::Impl(input));
            }
            expand_qt_methods_impl(input)
        }
        Item::Trait(mut input) => {
            if let Some(host_attr) = take_item_level_host_attr(&mut input.attrs)? {
                return super::host_attr::expand_qt_host_attr(host_attr, Item::Trait(input));
            }
            expand_qt_methods_trait_decl(input)
        }
        other => Err(syn::Error::new_spanned(
            other,
            "#[qt_methods] only supports traits and impl blocks",
        )),
    }
}

fn expand_qt_methods_impl(input: ItemImpl) -> syn::Result<proc_macro2::TokenStream> {
    if input.trait_.is_some() {
        return Err(syn::Error::new_spanned(
            input.self_ty,
            "#[qt_methods] trait impls should be handled by attachment path",
        ));
    }

    match prop_methods::classify_qt_method_impl(&input)? {
        QtMethodImplKind::None => {}
        QtMethodImplKind::Pure => return prop_methods::expand_qt_prop_methods_impl(input),
        QtMethodImplKind::Mixed => {
            let (prop_impl, host_impl) = split_mixed_impl(input)?;
            let host_tokens = expand_qt_host_methods_impl(host_impl)?;
            let prop_tokens = prop_methods::expand_qt_prop_methods_impl(prop_impl)?;
            return Ok(quote! {
                #host_tokens
                #prop_tokens
            });
        }
    }

    expand_qt_host_methods_impl(input)
}

fn split_mixed_impl(mut input: ItemImpl) -> syn::Result<(ItemImpl, ItemImpl)> {
    let mut prop_impl = input.clone();
    prop_impl.items.clear();

    let items = std::mem::take(&mut input.items);
    for item in items {
        let syn::ImplItem::Fn(method) = &item else {
            return Err(syn::Error::new_spanned(
                item,
                "#[qt_methods] only supports methods",
            ));
        };

        if prop_methods::method_uses_qt_prop_attrs(&method.attrs)? {
            prop_impl.items.push(item);
        } else {
            input.items.push(item);
        }
    }

    Ok((prop_impl, input))
}

fn expand_qt_methods_attach_impl(input: ItemImpl) -> syn::Result<proc_macro2::TokenStream> {
    let schema = widget_core_schema_path();
    let runtime = widget_core_runtime_path();
    let decl = widget_core_decl_path();
    expand_qt_methods_attach_impl_with_paths(input, &schema, &runtime, &decl)
}

fn expand_qt_methods_attach_impl_with_paths(
    input: ItemImpl,
    schema: &proc_macro2::TokenStream,
    runtime: &proc_macro2::TokenStream,
    decl: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let self_ty = input.self_ty.clone();
    let widget_ident = widget_self_ident(&self_ty)?;
    let (_, trait_path, _) = input
        .trait_
        .as_ref()
        .expect("trait attach path requires trait");
    let method_fragment = emit_widget_method_fragment(
        &schema,
        &decl,
        &widget_ident,
        quote!(<#self_ty as #trait_path>::__qt_method_spec_decl()),
    );
    let prop_decl_fragment = emit_widget_prop_decl_fragment(
        &schema,
        &runtime,
        &decl,
        &widget_ident,
        quote!(<#self_ty as #trait_path>::__qt_prop_decl()),
    );
    let prop_runtime_fragment = emit_widget_prop_runtime_fragment(
        &runtime,
        &decl,
        &widget_ident,
        quote!(<#self_ty as #trait_path>::__qt_prop_runtime_decl()),
    );
    let widget_decl_assert = quote! {
        const _: fn() = || {
            fn __qt_assert_widget_decl<T: #schema::QtWidgetDecl>() {}
            __qt_assert_widget_decl::<#self_ty>();
        };
    };

    Ok(quote! {
        #input

        #widget_decl_assert

        #method_fragment
        #prop_decl_fragment
        #prop_runtime_fragment
    })
}

fn expand_qt_methods_trait_decl(input: ItemTrait) -> syn::Result<proc_macro2::TokenStream> {
    let schema = widget_core_schema_path();
    let runtime = widget_core_runtime_path();
    expand_qt_methods_trait_decl_with_paths(input, &schema, &runtime)
}

fn expand_qt_methods_trait_decl_with_paths(
    mut input: ItemTrait,
    schema: &proc_macro2::TokenStream,
    runtime: &proc_macro2::TokenStream,
) -> syn::Result<proc_macro2::TokenStream> {
    let vis = input.vis.clone();
    let trait_ident = input.ident.clone();
    let helper_ident = format_ident!("__QtMethodSpecDeclFor{}", trait_ident);
    let trait_generics = input.generics.clone();
    let (trait_impl_generics, trait_ty_generics, trait_where_clause) =
        trait_generics.split_for_impl();
    let mut spec_host_methods = Vec::new();
    let mut manual_props = BTreeMap::<String, prop_methods::ManualPropEntry>::new();
    let mut next_setter_slot = 0u16;
    let mut next_getter_slot = 0u16;
    let mut retained_items = Vec::new();

    for mut item in std::mem::take(&mut input.items) {
        let TraitItem::Fn(method) = &mut item else {
            return Err(syn::Error::new_spanned(
                item,
                "#[qt_methods] trait declarations only support methods",
            ));
        };

        let has_qt_attrs = method.attrs.iter().any(|attr| attr.path().is_ident("qt"));
        if !has_qt_attrs {
            retained_items.push(item);
            continue;
        }

        if prop_methods::method_uses_qt_prop_attrs(&method.attrs)? {
            let prop_config = prop_methods::parse_qt_prop_method_config(&method.attrs)?;
            if let Some(default) = method.default.as_ref() {
                if qt_cpp_macro_body(default)?.is_some() {
                    return Err(syn::Error::new_spanned(
                        default,
                        "qt::cpp! requires #[qt(host)] on same method",
                    ));
                }
            }
            let prop_methods::QtPropMethodConfig::Prop(prop) = prop_config else {
                unreachable!("prop method parse should stay in prop branch");
            };
            let prop_type = match prop.kind {
                prop_methods::ManualPropKind::Setter => {
                    prop_methods::qt_prop_setter_value_type(&method.sig)?
                }
                prop_methods::ManualPropKind::Getter => {
                    prop_methods::qt_prop_getter_value_type(&method.sig)?
                }
            };
            let entry = manual_props
                .entry(prop.js_name.clone())
                .or_insert_with(|| prop_methods::ManualPropEntry::new(&prop.js_name));
            entry.record_value_type(&prop_type)?;
            if let Some(default) = prop.default.as_ref() {
                if entry.default.is_some() {
                    return Err(syn::Error::new_spanned(
                        default,
                        "duplicate widget prop default",
                    ));
                }
                entry.default = Some(prop_methods::spec_default_value_tokens(
                    &schema,
                    &prop_type,
                    Some(default),
                )?);
            }
            match prop.kind {
                prop_methods::ManualPropKind::Setter => {
                    prop_methods::validate_qt_prop_setter_signature(&method.sig)?;
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
                    let slot = next_setter_slot;
                    next_setter_slot = next_setter_slot.checked_add(1).ok_or_else(|| {
                        syn::Error::new_spanned(
                            &method.sig.ident,
                            "#[qt_methods] supports at most 65535 widget prop setters per trait",
                        )
                    })?;
                    *slot_ref = Some(slot);
                    entry.value_type = Some(prop_methods::qt_prop_value_type_tokens(
                        &schema, &prop_type,
                    )?);
                    let runtime_method = prop_methods::SetterRuntimeMethod {
                        ident: method.sig.ident.clone(),
                        value_type: prop_type.clone(),
                        returns_result: prop_methods::qt_prop_setter_returns_result(&method.sig)?,
                    };
                    if prop.init {
                        entry.init_setter_runtime = Some(runtime_method);
                    } else {
                        entry.setter_runtime = Some(runtime_method);
                    }
                }
                prop_methods::ManualPropKind::Getter => {
                    prop_methods::validate_qt_prop_getter_signature(&method.sig)?;
                    if entry.getter_slot.is_some() {
                        return Err(syn::Error::new_spanned(
                            &method.sig.ident,
                            "duplicate widget prop getter for same prop",
                        ));
                    }
                    let slot = next_getter_slot;
                    next_getter_slot = next_getter_slot.checked_add(1).ok_or_else(|| {
                        syn::Error::new_spanned(
                            &method.sig.ident,
                            "#[qt_methods] supports at most 65535 widget prop getters per trait",
                        )
                    })?;
                    entry.getter_slot = Some(slot);
                    entry.value_type = Some(prop_methods::qt_prop_value_type_tokens(
                        &schema, &prop_type,
                    )?);
                    entry.getter_runtime = Some(prop_methods::GetterRuntimeMethod {
                        ident: method.sig.ident.clone(),
                        value_type: prop_type.clone(),
                        returns_result: prop_methods::qt_prop_getter_returns_result(&method.sig)?,
                    });
                }
            }
            method.attrs.retain(|attr| !attr.path().is_ident("qt"));
            retained_items.push(item);
            continue;
        }

        let method_config = parse_method_config(&method.attrs)?;
        method.attrs.retain(|attr| !attr.path().is_ident("qt"));

        match method_config.lowering {
            MethodLoweringConfig::Plain => {
                if let Some(default) = method.default.as_ref() {
                    if qt_cpp_macro_body(default)?.is_some() {
                        return Err(syn::Error::new_spanned(
                            default,
                            "qt::cpp! requires #[qt(host)] on same method",
                        ));
                    }
                }
                retained_items.push(item);
            }
            MethodLoweringConfig::Host { host_name_override } => {
                if !method_config.extra_includes.is_empty() {
                    return Err(syn::Error::new_spanned(
                        &method.sig,
                        "#[qt(include = ...)] is only valid with qt::cpp! bodies",
                    ));
                }
                if let Some(default) = method.default.as_ref() {
                    if qt_cpp_macro_body(default)?.is_some() {
                        return Err(syn::Error::new_spanned(
                            default,
                            "qt::cpp! requires #[qt(host)] trait capability, not #[qt_methods]",
                        ));
                    }
                }
                let receiver_kind = host_method_receiver_kind(&method.sig)?;
                let host_name = host_name_override
                    .unwrap_or_else(|| snake_to_lower_camel(&method.sig.ident.to_string()));
                let slot = u16::try_from(spec_host_methods.len()).map_err(|_| {
                    syn::Error::new_spanned(
                        &method.sig.ident,
                        "#[qt_methods] supports at most 65535 host methods per trait",
                    )
                })?;
                add_trait_host_owner_bound(&mut method.sig, &runtime, receiver_kind);
                if method.default.is_none() {
                    let body = expand_host_method_body(
                        &runtime,
                        &method.sig,
                        slot,
                        &host_name,
                        receiver_kind,
                    )?;
                    method.default = Some(syn::parse2(body)?);
                }
                spec_host_methods.push(build_host_method_spec_tokens(
                    &schema,
                    &method.sig,
                    &host_name,
                    slot,
                )?);
                retained_items.push(item);
            }
        }
    }

    input.items = retained_items;
    let manual_prop_values = manual_props.values().collect::<Vec<_>>();
    let prop_decl_specs = manual_prop_values
        .iter()
        .map(|prop| prop.spec_tokens(&schema))
        .collect::<syn::Result<Vec<_>>>()?;
    let prop_runtime_decl = build_trait_prop_runtime_decl(
        &runtime,
        quote!(#trait_ident #trait_ty_generics),
        manual_prop_values.as_slice(),
    )?;
    input.items.push(syn::parse_quote! {
        #[doc(hidden)]
        fn __qt_prop_decl() -> &'static [#schema::SpecPropDecl]
        where
            Self: Sized,
        {
            <() as #helper_ident #trait_ty_generics>::prop_decl_items()
        }
    });
    input.items.push(syn::parse_quote! {
        #[doc(hidden)]
        fn __qt_prop_runtime_decl() -> &'static #runtime::WidgetPropRuntimeDecl
        where
            Self: Sized,
        {
            #prop_runtime_decl
        }
    });
    input.items.push(syn::parse_quote! {
        #[doc(hidden)]
        fn __qt_method_spec_decl() -> &'static #schema::SpecMethodSet
        where
            Self: Sized,
        {
            <() as #helper_ident #trait_ty_generics>::decl()
        }
    });

    Ok(quote! {
        #input

        #[doc(hidden)]
        #vis trait #helper_ident #trait_generics {
            fn prop_decl_items() -> &'static [#schema::SpecPropDecl];
            fn decl() -> &'static #schema::SpecMethodSet;
        }

        impl #trait_impl_generics #helper_ident #trait_ty_generics for () #trait_where_clause {
            fn prop_decl_items() -> &'static [#schema::SpecPropDecl] {
                static DECL: &[#schema::SpecPropDecl] = &[
                    #(#prop_decl_specs,)*
                ];

                DECL
            }

            fn decl() -> &'static #schema::SpecMethodSet {
                static DECL: #schema::SpecMethodSet =
                    #schema::SpecMethodSet {
                        host_methods: &[
                            #(#spec_host_methods,)*
                        ],
                    };
                &DECL
            }
        }
    })
}

fn expand_qt_host_methods_impl(mut input: ItemImpl) -> syn::Result<proc_macro2::TokenStream> {
    let schema = widget_core_schema_path();
    let codegen = widget_core_codegen_path();
    let runtime = widget_core_runtime_path();
    let decl = widget_core_decl_path();
    let self_ty = input.self_ty.clone();
    let impl_generics = input.generics.clone();
    let (_, _, where_clause) = input.generics.split_for_impl();
    let mut spec_host_methods = Vec::new();
    let mut opaque_codegen_methods = Vec::new();
    let mut retained_items = Vec::new();

    for mut item in std::mem::take(&mut input.items) {
        let syn::ImplItem::Fn(method) = &mut item else {
            return Err(syn::Error::new_spanned(
                item,
                "#[qt_methods] only supports methods",
            ));
        };

        let method_config = parse_method_config(&method.attrs)?;
        method.attrs.retain(|attr| !attr.path().is_ident("qt"));
        let cpp_body = qt_cpp_macro_body(&method.block)?;
        let codegen_spec = cpp_body.map(|body| OpaqueCodegenLoweringConfig {
            extra_includes: method_config.extra_includes.clone(),
            body,
        });

        match method_config.lowering {
            MethodLoweringConfig::Plain => {
                if codegen_spec.is_some() {
                    return Err(syn::Error::new_spanned(
                        &method.block,
                        "qt::cpp! requires #[qt(host)] on the same method",
                    ));
                }
                retained_items.push(item);
            }
            MethodLoweringConfig::Host { host_name_override } => {
                if codegen_spec.is_none() && !method_config.extra_includes.is_empty() {
                    return Err(syn::Error::new_spanned(
                        &method.block,
                        "#[qt(include = ...)] requires a qt::cpp! body",
                    ));
                }
                let receiver_kind = host_method_receiver_kind(&method.sig)?;
                let host_name = host_name_override
                    .unwrap_or_else(|| snake_to_lower_camel(&method.sig.ident.to_string()));
                let slot = u16::try_from(spec_host_methods.len()).map_err(|_| {
                    syn::Error::new_spanned(
                        &method.sig.ident,
                        "#[qt_methods] supports at most 65535 host methods per impl",
                    )
                })?;
                let body = expand_host_method_body(
                    &runtime,
                    &method.sig,
                    slot,
                    &host_name,
                    receiver_kind,
                )?;
                method.block = syn::parse2(body)?;

                let rust_name = LitStr::new(&method.sig.ident.to_string(), method.sig.ident.span());
                let _ = rust_name;
                spec_host_methods.push(build_host_method_spec_tokens(
                    &schema,
                    &method.sig,
                    &host_name,
                    slot,
                )?);

                let lowering_tokens = opaque_codegen_spec_tokens(&codegen, codegen_spec.as_ref());
                opaque_codegen_methods.push(quote! {
                    #codegen::OpaqueMethodCodegenMeta {
                        slot: #slot,
                        lowering: #lowering_tokens,
                    }
                });

                retained_items.push(item);
            }
        }
    }

    input.items = retained_items;
    let widget_method_fragment = if input.generics.params.is_empty() {
        let widget_ident = widget_self_ident(&self_ty)?;
        Some(emit_widget_method_fragment(
            &schema,
            &decl,
            &widget_ident,
            quote!(&<#self_ty as #schema::QtMethodSet>::SPEC),
        ))
    } else {
        None
    };

    Ok(quote! {
        #input

        impl #impl_generics #schema::QtHostMethodSurface for #self_ty #where_clause {
            const SPEC: #schema::SpecHostMethodSet =
                #schema::SpecHostMethodSet {
                    methods: &[
                        #(#spec_host_methods,)*
                    ],
                };
        }

        impl #impl_generics #schema::QtMethodSet for #self_ty #where_clause {
            const SPEC: #schema::SpecMethodSet =
                #schema::SpecMethodSet {
                    host_methods: <Self as #schema::QtHostMethodSurface>::SPEC.methods,
                };
        }

        impl #impl_generics #codegen::QtOpaqueMethodCodegenSurface for #self_ty #where_clause {
            const CODEGEN: #codegen::OpaqueMethodCodegenSet =
                #codegen::OpaqueMethodCodegenSet {
                    methods: &[
                        #(#opaque_codegen_methods,)*
                    ],
                };
        }

        impl #impl_generics #runtime::QtOpaqueHookDispatchSurface<#self_ty> for #self_ty #where_clause {
            fn __qt_invoke_opaque_hook(
                this: &mut #self_ty,
                hook_name: &str,
                host: &mut dyn #runtime::QtOpaqueHostMutDyn,
            ) -> #runtime::WidgetResult<()> {
                let _ = (this, host);
                match hook_name {
                    _ => Err(#runtime::WidgetError::new(format!(
                        "widget opaque hook {hook_name} is not registered"
                    ))),
                }
            }
        }

        #widget_method_fragment
    })
}

fn opaque_codegen_spec_tokens(
    codegen: &proc_macro2::TokenStream,
    spec: Option<&OpaqueCodegenLoweringConfig>,
) -> proc_macro2::TokenStream {
    let Some(spec) = spec else {
        return quote!(None);
    };
    let includes = spec.extra_includes.iter();
    let body = &spec.body;

    quote! {
        Some(#codegen::OpaqueCodegenLowering {
            extra_includes: &[
                #(#includes,)*
            ],
            body: #body,
        })
    }
}

fn host_method_receiver_kind(sig: &syn::Signature) -> syn::Result<HostMethodReceiverKind> {
    let Some(first_arg) = sig.inputs.first() else {
        return Err(syn::Error::new_spanned(
            sig,
            "#[qt_methods] host methods require a receiver",
        ));
    };

    let FnArg::Receiver(receiver) = first_arg else {
        return Err(syn::Error::new_spanned(
            first_arg,
            "#[qt_methods] host methods require self receiver",
        ));
    };

    if receiver.reference.is_none() || receiver.colon_token.is_some() {
        return Err(syn::Error::new_spanned(
            first_arg,
            "#[qt_methods] host methods require &self or &mut self receiver",
        ));
    }

    Ok(if receiver.mutability.is_some() {
        HostMethodReceiverKind::Mutable
    } else {
        HostMethodReceiverKind::Shared
    })
}

fn method_return_value_type_tokens(output: &ReturnType) -> syn::Result<proc_macro2::TokenStream> {
    let Some(ty) = method_return_type(output)? else {
        return Ok(quote!(()));
    };

    if is_unit_type(ty) {
        return Ok(quote!(()));
    }

    Ok(quote!(#ty))
}

fn method_returns_result(output: &ReturnType) -> syn::Result<bool> {
    let ty = match output {
        ReturnType::Default => return Ok(false),
        ReturnType::Type(_, ty) => ty.as_ref(),
    };
    Ok(unwrap_result_type(ty)?.is_some())
}

fn expand_host_method_body(
    runtime: &proc_macro2::TokenStream,
    sig: &syn::Signature,
    slot: u16,
    host_name: &str,
    receiver_kind: HostMethodReceiverKind,
) -> syn::Result<proc_macro2::TokenStream> {
    let return_ty = method_return_value_type_tokens(&sig.output)?;
    let returns_result = method_returns_result(&sig.output)?;
    let host_name_lit = LitStr::new(host_name, sig.ident.span());
    let call_expr = match receiver_kind {
        HostMethodReceiverKind::Shared => {
            quote!(<Self as #runtime::QtHostMethodOwner>::__qt_call_host_method(
                self,
                #slot,
                #host_name_lit,
                __qt_args,
            ))
        }
        HostMethodReceiverKind::Mutable => {
            quote!(<Self as #runtime::QtHostMethodOwnerMut>::__qt_call_host_method_mut(
                self,
                #slot,
                #host_name_lit,
                __qt_args,
            ))
        }
    };

    let arg_pushes = sig
        .inputs
        .iter()
        .skip(1)
        .map(|arg| {
            let FnArg::Typed(arg) = arg else {
                return Err(syn::Error::new_spanned(
                    arg,
                    "#[qt_methods] host methods only support named value arguments",
                ));
            };
            let Pat::Ident(pat_ident) = arg.pat.as_ref() else {
                return Err(syn::Error::new_spanned(
                    &arg.pat,
                    "#[qt_methods] method arguments require simple identifiers",
                ));
            };
            if option_inner_type(&arg.ty).is_some() {
                return Err(syn::Error::new_spanned(
                    &arg.ty,
                    "#[qt_methods] does not support Option<T> host method arguments",
                ));
            }
            let ident = &pat_ident.ident;
            Ok(quote! {
                __qt_args.push(#runtime::IntoQt::into_qt(#ident)?);
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;

    let result_expr = if returns_result {
        quote! {
            {
                let mut __qt_args = std::vec::Vec::new();
                #(#arg_pushes)*
                let __qt_value = #call_expr?;
                <#return_ty as #runtime::TryFromQt>::try_from_qt(__qt_value)
            }
        }
    } else {
        quote! {
            {
                let __qt_result = (|| -> #runtime::WidgetResult<#return_ty> {
                    let mut __qt_args = std::vec::Vec::new();
                    #(#arg_pushes)*
                    let __qt_value = #call_expr?;
                    <#return_ty as #runtime::TryFromQt>::try_from_qt(__qt_value)
                })();
                match __qt_result {
                    Ok(value) => value,
                    Err(error) => panic!("qt host method {} failed: {}", #host_name_lit, error),
                }
            }
        }
    };

    Ok(quote!({ #result_expr }))
}

fn method_arg_type_tokens(ty: &Type) -> syn::Result<proc_macro2::TokenStream> {
    let schema = widget_core_schema_path();
    if option_inner_type(ty).is_some() {
        return Err(syn::Error::new_spanned(
            ty,
            "#[qt_methods] does not support Option<T> method arguments",
        ));
    }

    Ok(quote!(<#ty as #schema::QtType>::INFO))
}

fn method_return_type_tokens(output: &ReturnType) -> syn::Result<proc_macro2::TokenStream> {
    let schema = widget_core_schema_path();
    let Some(ty) = method_return_type(output)? else {
        return Ok(quote!(<() as #schema::QtType>::INFO));
    };

    if is_unit_type(ty) {
        return Ok(quote!(<() as #schema::QtType>::INFO));
    }

    Ok(quote!(<#ty as #schema::QtType>::INFO))
}

fn build_host_method_spec_tokens(
    schema: &proc_macro2::TokenStream,
    sig: &syn::Signature,
    host_name: &str,
    slot: u16,
) -> syn::Result<proc_macro2::TokenStream> {
    let rust_name = LitStr::new(&sig.ident.to_string(), sig.ident.span());
    let js_name = LitStr::new(
        &snake_to_lower_camel(&sig.ident.to_string()),
        sig.ident.span(),
    );
    let host_name = LitStr::new(host_name, sig.ident.span());
    let return_type = method_return_type_tokens(&sig.output)?;
    let mut args = Vec::new();

    for arg in sig.inputs.iter().skip(1) {
        let FnArg::Typed(arg) = arg else {
            return Err(syn::Error::new_spanned(
                arg,
                "#[qt_methods] host methods only support named value arguments",
            ));
        };

        let Pat::Ident(pat_ident) = arg.pat.as_ref() else {
            return Err(syn::Error::new_spanned(
                &arg.pat,
                "#[qt_methods] method arguments require simple identifiers",
            ));
        };

        let arg_name = LitStr::new(&pat_ident.ident.to_string(), pat_ident.ident.span());
        let arg_type = method_arg_type_tokens(&arg.ty)?;
        args.push(quote! {
            #schema::SpecHostMethodArg {
                rust_name: #arg_name,
                js_name: #arg_name,
                value_type: #arg_type,
            }
        });
    }

    Ok(quote! {
        #schema::SpecHostMethodMeta {
            slot: #slot,
            rust_name: #rust_name,
            js_name: #js_name,
            host_name: #host_name,
            args: &[
                #(#args,)*
            ],
            return_type: #return_type,
        }
    })
}

fn emit_widget_method_fragment(
    schema: &proc_macro2::TokenStream,
    decl: &proc_macro2::TokenStream,
    widget_ident: &Ident,
    methods: proc_macro2::TokenStream,
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
                            props: &[],
                            events: &[],
                            methods: #methods,
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

fn emit_widget_prop_decl_fragment(
    schema: &proc_macro2::TokenStream,
    runtime: &proc_macro2::TokenStream,
    decl: &proc_macro2::TokenStream,
    widget_ident: &Ident,
    props: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        const _: () = {
            use #runtime::linkme::distributed_slice;

            fn __qt_prop_decl() -> &'static [#schema::SpecPropDecl] {
                #props
            }

            #[distributed_slice(#runtime::QT_WIDGET_PROP_DECL_FRAGMENTS)]
            #[linkme(crate = #runtime::linkme)]
            static __QT_WIDGET_PROP_FRAGMENT: &#runtime::WidgetPropDeclFragment =
                &#runtime::WidgetPropDeclFragment {
                    spec_key: #decl::SpecWidgetKey::new(concat!(
                        module_path!(),
                        "::",
                        stringify!(#widget_ident)
                    )),
                    decl: __qt_prop_decl,
                };
        };
    }
}

fn emit_widget_prop_runtime_fragment(
    runtime: &proc_macro2::TokenStream,
    decl: &proc_macro2::TokenStream,
    widget_ident: &Ident,
    runtime_decl: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    quote! {
        const _: () = {
            use #runtime::linkme::distributed_slice;

            fn __qt_prop_runtime_decl() -> &'static #runtime::WidgetPropRuntimeDecl {
                #runtime_decl
            }

            #[distributed_slice(#runtime::QT_WIDGET_PROP_RUNTIME_FRAGMENTS)]
            #[linkme(crate = #runtime::linkme)]
            static __QT_WIDGET_PROP_RUNTIME_FRAGMENT: &#runtime::WidgetPropRuntimeFragment =
                &#runtime::WidgetPropRuntimeFragment {
                    spec_key: #decl::SpecWidgetKey::new(concat!(
                        module_path!(),
                        "::",
                        stringify!(#widget_ident)
                    )),
                    decl: __qt_prop_runtime_decl,
                };
        };
    }
}

fn build_trait_prop_runtime_decl(
    runtime: &proc_macro2::TokenStream,
    trait_path: proc_macro2::TokenStream,
    props: &[&prop_methods::ManualPropEntry],
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
            let helper_name = format_ident!(
                "__qt_prop_setter_{}",
                method.ident,
                span = method.ident.span()
            );
            let method_ident = &method.ident;
            let js_name = LitStr::new(&prop.js_name, proc_macro2::Span::call_site());
            let ty = option_inner_type(&method.value_type).unwrap_or(&method.value_type);
            let body = if method.returns_result {
                quote! {
                    let widget = unsafe { &mut *(raw.cast::<Self>()) };
                    let value = <#ty as #runtime::TryFromQt>::try_from_qt(value)?;
                    <Self as #trait_path>::#method_ident(widget, value)
                }
            } else {
                quote! {
                    let widget = unsafe { &mut *(raw.cast::<Self>()) };
                    let value = <#ty as #runtime::TryFromQt>::try_from_qt(value)?;
                    <Self as #trait_path>::#method_ident(widget, value);
                    Ok(())
                }
            };
            setter_helpers.push(quote! {
                unsafe fn #helper_name(
                    raw: *mut (),
                    value: #runtime::QtValue,
                ) -> #runtime::WidgetResult<()> {
                    #body
                }
            });
            setter_meta.push(quote! {
                #runtime::WidgetPropSetterRuntimeMeta {
                    js_name: #js_name,
                    invoke: #helper_name,
                }
            });
        }

        if let Some(method) = prop.getter_runtime.as_ref() {
            let helper_name = format_ident!(
                "__qt_prop_getter_{}",
                method.ident,
                span = method.ident.span()
            );
            let method_ident = &method.ident;
            let js_name = LitStr::new(&prop.js_name, proc_macro2::Span::call_site());
            let ty = option_inner_type(&method.value_type).unwrap_or(&method.value_type);
            let body = if method.returns_result {
                quote! {
                    let widget = unsafe { &*(raw.cast::<Self>()) };
                    let value = <Self as #trait_path>::#method_ident(widget)?;
                    <#ty as #runtime::IntoQt>::into_qt(value)
                }
            } else {
                quote! {
                    let widget = unsafe { &*(raw.cast::<Self>()) };
                    let value = <Self as #trait_path>::#method_ident(widget);
                    <#ty as #runtime::IntoQt>::into_qt(value)
                }
            };
            getter_helpers.push(quote! {
                unsafe fn #helper_name(raw: *const ()) -> #runtime::WidgetResult<#runtime::QtValue> {
                    #body
                }
            });
            getter_meta.push(quote! {
                #runtime::WidgetPropGetterRuntimeMeta {
                    js_name: #js_name,
                    invoke: #helper_name,
                }
            });
        }
    }

    Ok(quote! {{
        #(#setter_helpers)*
        #(#getter_helpers)*

        std::boxed::Box::leak(std::boxed::Box::new(#runtime::WidgetPropRuntimeDecl {
            setters: std::boxed::Box::leak(std::boxed::Box::new([
                #(#setter_meta,)*
            ])),
            getters: std::boxed::Box::leak(std::boxed::Box::new([
                #(#getter_meta,)*
            ])),
        }))
    }})
}

fn add_trait_host_owner_bound(
    sig: &mut syn::Signature,
    runtime: &proc_macro2::TokenStream,
    receiver_kind: HostMethodReceiverKind,
) {
    let where_clause = sig.generics.make_where_clause();
    let predicate: syn::WherePredicate = match receiver_kind {
        HostMethodReceiverKind::Shared => syn::parse_quote!(Self: #runtime::QtHostMethodOwner),
        HostMethodReceiverKind::Mutable => syn::parse_quote!(Self: #runtime::QtHostMethodOwnerMut),
    };
    where_clause.predicates.push(predicate);
}

fn widget_self_ident(self_ty: &Type) -> syn::Result<Ident> {
    let Type::Path(type_path) = self_ty else {
        return Err(syn::Error::new_spanned(
            self_ty,
            "#[qt_methods] requires a named widget type",
        ));
    };
    let Some(last) = type_path.path.segments.last() else {
        return Err(syn::Error::new_spanned(
            self_ty,
            "#[qt_methods] requires a named widget type",
        ));
    };
    Ok(last.ident.clone())
}

#[cfg(test)]
mod tests {
    use super::*;
    use syn::parse_quote;

    #[test]
    fn mixed_impl_is_split_and_expands() {
        let item: ItemImpl = parse_quote! {
            impl DemoWidget {
                #[qt(constructor)]
                fn create() -> Self {
                    Self
                }

                #[qt(host)]
                fn focus(&self) {}
            }
        };

        let (prop_impl, host_impl) = split_mixed_impl(item).expect("mixed impl splits");
        assert_eq!(prop_impl.items.len(), 1);
        assert_eq!(host_impl.items.len(), 1);
    }

    #[test]
    fn trait_prop_methods_emit_prop_helpers() {
        let runtime = quote::quote!(runtime);
        let mut prop = prop_methods::ManualPropEntry::new("width");
        prop.record_value_type(&parse_quote!(i32))
            .expect("manual prop type");
        prop.value_type = Some(quote::quote!(<i32 as schema::QtType>::INFO));
        prop.setter_slot = Some(0);
        prop.getter_slot = Some(0);
        prop.setter_runtime = Some(prop_methods::SetterRuntimeMethod {
            ident: parse_quote!(set_width),
            value_type: parse_quote!(i32),
            returns_result: false,
        });
        prop.getter_runtime = Some(prop_methods::GetterRuntimeMethod {
            ident: parse_quote!(width),
            value_type: parse_quote!(i32),
            returns_result: false,
        });

        let expanded = build_trait_prop_runtime_decl(&runtime, quote::quote!(DemoProps), &[&prop])
            .expect("trait prop runtime decl builds")
            .to_string();

        assert!(expanded.contains("WidgetPropRuntimeDecl"));
        assert!(expanded.contains("Self as DemoProps"));
        assert!(expanded.contains("set_width"));
        assert!(expanded.contains("width"));
    }

    #[test]
    fn attach_impl_emits_prop_and_runtime_fragments() {
        let schema = quote::quote!(schema);
        let runtime = quote::quote!(runtime);
        let decl = quote::quote!(decl);
        let widget_ident = format_ident!("DemoWidget");
        let prop_fragment = emit_widget_prop_decl_fragment(
            &schema,
            &runtime,
            &decl,
            &widget_ident,
            quote::quote!(demo_props),
        );
        let runtime_fragment = emit_widget_prop_runtime_fragment(
            &runtime,
            &decl,
            &widget_ident,
            quote::quote!(demo_runtime),
        );
        let expanded = quote::quote! {
            #prop_fragment
            #runtime_fragment
        }
        .to_string();

        assert!(expanded.contains("QT_WIDGET_PROP_DECL_FRAGMENTS"));
        assert!(expanded.contains("QT_WIDGET_PROP_RUNTIME_FRAGMENTS"));
        assert!(!expanded.contains("QT_WIDGET_PROP_SPEC_FRAGMENTS"));
    }

    #[test]
    fn attach_impl_emits_widget_decl_assertion() {
        let item: ItemImpl = parse_quote! {
            impl DemoProps for DemoWidget {}
        };

        let expanded = expand_qt_methods_attach_impl_with_paths(
            item,
            &quote!(schema),
            &quote!(runtime),
            &quote!(decl),
        )
        .expect("attach impl expands")
        .to_string();

        assert!(expanded.contains("QtWidgetDecl"));
        assert!(expanded.contains("__qt_assert_widget_decl"));
    }

    #[test]
    fn take_item_level_qt_host_attr_strips_trait_marker() {
        let mut item: ItemTrait = parse_quote! {
            #[qt(host(class = "QWidget", include = "<QtWidgets/QWidget>"))]
            trait DemoHost {}
        };

        let host_attr =
            take_item_level_host_attr(&mut item.attrs).expect("item-level host attr parses");
        let rendered = host_attr.expect("host attr tokens").to_string();

        assert!(rendered.contains("class"));
        assert!(rendered.contains("QWidget"));
        assert!(rendered.contains("include"));
        assert!(item.attrs.is_empty());
    }

    #[test]
    fn take_item_level_qt_host_attr_strips_attach_marker() {
        let mut item: ItemImpl = parse_quote! {
            #[qt(host)]
            impl DemoHost for DemoWidget {}
        };

        let host_attr =
            take_item_level_host_attr(&mut item.attrs).expect("attach host attr parses");

        assert!(host_attr.expect("empty host attr").is_empty());
        assert!(item.attrs.is_empty());
    }

    #[test]
    fn trait_prop_methods_reject_duplicate_live_setter() {
        let item: ItemTrait = parse_quote! {
            trait DemoProps {
                #[qt(prop = width, setter)]
                fn set_width(&mut self, value: i32);

                #[qt(prop = width, setter)]
                fn set_width_again(&mut self, value: i32);
            }
        };

        let error =
            expand_qt_methods_trait_decl_with_paths(item, &quote!(schema), &quote!(runtime))
                .err()
                .expect("duplicate setter must fail");

        assert!(
            error
                .to_string()
                .contains("duplicate widget prop live setter for same prop")
        );
    }

    #[test]
    fn trait_prop_methods_reject_duplicate_defaults() {
        let item: ItemTrait = parse_quote! {
            trait DemoProps {
                #[qt(prop = width, setter, default = 1)]
                fn set_width(&mut self, value: i32);

                #[qt(prop = width, getter, default = 2)]
                fn width(&self) -> i32;
            }
        };

        let error =
            expand_qt_methods_trait_decl_with_paths(item, &quote!(schema), &quote!(runtime))
                .err()
                .expect("duplicate defaults must fail");

        assert!(error.to_string().contains("duplicate widget prop default"));
    }

    #[test]
    fn trait_prop_methods_reject_type_conflicts() {
        let item: ItemTrait = parse_quote! {
            trait DemoProps {
                #[qt(prop = width, setter)]
                fn set_width(&mut self, value: i32);

                #[qt(prop = width, getter)]
                fn width(&self) -> bool;
            }
        };

        let error =
            expand_qt_methods_trait_decl_with_paths(item, &quote!(schema), &quote!(runtime))
                .err()
                .expect("mixed prop types must fail");

        assert!(
            error
                .to_string()
                .contains("mixes incompatible Rust value types")
        );
    }
}
