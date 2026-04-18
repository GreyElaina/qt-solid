mod model;
mod parse;

use self::model::HostConfig;
use self::parse::{
    parse_widget_decl_config, parse_widget_field_default, resolve_widget_export_name,
};
use crate::common::{widget_core_decl_path, widget_core_runtime_path, widget_core_schema_path};
use crate::props;
use quote::{format_ident, quote};
use syn::{Fields, Ident, ItemStruct, LitStr, Token, punctuated::Punctuated};

pub(crate) fn expand_qt_widget_attr(
    meta_items: Punctuated<syn::Meta, Token![,]>,
    input: ItemStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    expand_qt_widget_decl(meta_items, input)
}

fn expand_qt_widget_decl(
    meta_items: Punctuated<syn::Meta, Token![,]>,
    input: ItemStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    let struct_item = input.clone();
    let visibility = input.vis.clone();
    let struct_name = input.ident;

    if !input.generics.params.is_empty() {
        return Err(syn::Error::new_spanned(
            input.generics,
            "#[qt_entity(widget)] does not support generics",
        ));
    }

    let raw_user_fields = match input.fields {
        Fields::Unit => Vec::new(),
        Fields::Named(fields) => fields.named.iter().cloned().collect::<Vec<_>>(),
        Fields::Unnamed(fields) if fields.unnamed.is_empty() => Vec::new(),
        other => {
            return Err(syn::Error::new_spanned(
                other,
                "#[qt_entity(widget)] expects a unit or named-field struct",
            ));
        }
    };

    let field_defaults = raw_user_fields
        .iter()
        .map(parse_widget_field_default)
        .collect::<syn::Result<Vec<_>>>()?;
    let declares_props = props::struct_declares_widget_props(&raw_user_fields)?;
    let has_default_constructor = field_defaults.iter().all(|field| field.default.is_some());
    let default_field_inits = if has_default_constructor {
        field_defaults
            .iter()
            .map(|field| {
                let ident = &field.ident;
                let default = field
                    .default
                    .as_ref()
                    .expect("default constructor only emitted when every field has a default");
                quote!(#ident: #default)
            })
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let config = parse_widget_decl_config(meta_items)?;
    let decl = widget_core_decl_path();
    let schema = widget_core_schema_path();
    let runtime = widget_core_runtime_path();
    let type_name = LitStr::new(&struct_name.to_string(), struct_name.span());
    let kind_name = resolve_widget_export_name(&struct_name, &config);
    let children = config.children;
    let base_props_ty = if declares_props {
        quote!(#struct_name)
    } else {
        quote!(#schema::NoProps)
    };
    let props_ty = format_ident!("__QtPropsFor{}", struct_name);
    let host_spec_fragment =
        emit_widget_host_spec_fragment(&schema, &decl, &struct_name, config.host.as_ref());
    let spec_key_tokens = quote!(#decl::SpecWidgetKey::new(concat!(
        module_path!(),
        "::",
        stringify!(#struct_name)
    )));
    let native_factory_name = format_ident!("__qt_create_native_instance");
    let default_constructor_impl = if has_default_constructor {
        Some(quote! {
            impl #runtime::QtWidgetDefaultConstruct for #struct_name {
                fn __qt_default_construct() -> Self {
                    Self { #(#default_field_inits,)* }
                }
            }
        })
    } else {
        None
    };
    let native_decl_impl = if has_default_constructor {
        Some(quote! {
            fn #native_factory_name(
                handle: #runtime::WidgetHandle,
                _create_props: &[#runtime::WidgetCreateProp],
            ) -> #runtime::WidgetResult<std::sync::Arc<dyn #runtime::QtWidgetInstanceDyn>> {
                let widget = <#struct_name as #runtime::QtWidgetDefaultConstruct>::__qt_default_construct();
                Ok(#runtime::new_widget_instance::<#struct_name, #schema::NoMethods>(
                    handle,
                    widget,
                    #runtime::resolve_widget_host_behavior(#spec_key_tokens),
                    #runtime::resolve_widget_prop_runtime(#spec_key_tokens),
                ))
            }

            impl #runtime::QtWidgetNativeDecl for #struct_name {
                const NATIVE_DECL: #runtime::WidgetNativeDecl = #runtime::WidgetNativeDecl {
                    spec_key: #spec_key_tokens,
                    create_instance: #native_factory_name,
                };
            }
        })
    } else {
        None
    };

    let generated = quote! {
        #struct_item

        impl #struct_name {
            #[doc(hidden)]
            fn __qt_handle(&self) -> #runtime::WidgetResult<#runtime::WidgetHandle> {
                #runtime::widget_handle_for(self)
            }
        }

        #default_constructor_impl

        impl #runtime::WidgetHandleOwner for #struct_name {
            fn widget_handle(&self) -> #runtime::WidgetHandle {
                self.__qt_handle()
                    .expect("qt widget handle accessed before widget was attached to runtime")
            }
        }

        impl #runtime::QtHostMethodOwner for #struct_name {
            fn __qt_call_host_method(
                &self,
                slot: u16,
                name: &str,
                args: std::vec::Vec<#runtime::QtValue>,
            ) -> #runtime::WidgetResult<#runtime::QtValue> {
                let _ = slot;
                self.__qt_handle()?.call_host_method(name, &args)
            }
        }

        #[doc(hidden)]
        #[derive(Clone, Copy, Default)]
        #visibility struct #props_ty;

        impl #schema::QtPropTree for #props_ty {
            fn spec() -> &'static #schema::SpecPropTree {
                static SPEC: std::sync::OnceLock<&'static #schema::SpecPropTree> =
                    std::sync::OnceLock::new();

                SPEC.get_or_init(|| {
                    #schema::resolve_widget_prop_spec(
                        #spec_key_tokens,
                        #type_name,
                        <#base_props_ty as #schema::QtPropTree>::spec(),
                    )
                })
            }
        }

        impl #schema::QtWidgetDecl for #struct_name {
            type Props = #props_ty;

            fn spec() -> &'static #schema::SpecWidgetBinding {
                static CORE: std::sync::OnceLock<#schema::SpecWidgetCore> =
                    std::sync::OnceLock::new();
                static SPEC: std::sync::OnceLock<#schema::SpecWidgetBinding> =
                    std::sync::OnceLock::new();

                SPEC.get_or_init(|| #schema::resolve_widget_spec(
                    CORE.get_or_init(|| #schema::SpecWidgetCore {
                        spec_key: #decl::SpecWidgetKey::new(concat!(
                            module_path!(),
                            "::",
                            stringify!(#struct_name)
                        )),
                        kind_name: #kind_name,
                        type_name: #type_name,
                        children: #schema::ChildrenKind::#children,
                        props: <<Self as #schema::QtWidgetDecl>::Props as #schema::QtPropTree>::spec(),
                    })
                ))
            }

            fn binding() -> &'static #schema::WidgetBinding {
                static BINDING: std::sync::OnceLock<#schema::WidgetBinding> =
                    std::sync::OnceLock::new();

                BINDING.get_or_init(|| #schema::local_widget_binding(Self::spec()))
            }
        }

        #native_decl_impl

        #host_spec_fragment
    };

    Ok(generated)
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

fn emit_widget_host_spec_fragment(
    schema: &proc_macro2::TokenStream,
    decl: &proc_macro2::TokenStream,
    widget_ident: &Ident,
    host: Option<&HostConfig>,
) -> proc_macro2::TokenStream {
    let host_tokens = host
        .map(expand_host_meta)
        .map(|host| quote!(Some(#host)))
        .unwrap_or_else(|| quote!(None));

    quote! {
        const _: () = {
            use #schema::linkme::distributed_slice;

            fn __qt_widget_host_spec_decl() -> &'static #schema::HostCapabilitySpecDecl {
                static DECL: #schema::HostCapabilitySpecDecl = #schema::HostCapabilitySpecDecl {
                    host: #host_tokens,
                    default_layout: None,
                    props: &[],
                    events: &[],
                    methods: &#schema::NO_METHODS,
                };
                &DECL
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
                    decl: __qt_widget_host_spec_decl,
                };
        };
    }
}

pub(crate) fn emit_widget_codegen_fragment(
    codegen: &proc_macro2::TokenStream,
    decl: &proc_macro2::TokenStream,
    widget_ident: &Ident,
    overrides: &[proc_macro2::TokenStream],
    event_mounts: &[proc_macro2::TokenStream],
    prop_setters: &[proc_macro2::TokenStream],
    prop_getters: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    quote! {
        const _: () = {
            use #codegen::linkme::distributed_slice;

            #[distributed_slice(#codegen::QT_WIDGET_CODEGEN_FRAGMENTS)]
            #[linkme(crate = #codegen::linkme)]
            static __QT_WIDGET_CODEGEN_FRAGMENT: &#codegen::WidgetCodegenFragment = &#codegen::WidgetCodegenFragment {
                spec_key: #decl::SpecWidgetKey::new(concat!(
                    module_path!(),
                    "::",
                    stringify!(#widget_ident)
                )),
                decl: __qt_widget_codegen_decl,
            };

            fn __qt_widget_codegen_decl() -> &'static #codegen::HostCapabilityCodegenDecl {
                static DECL: #codegen::HostCapabilityCodegenDecl =
                    #codegen::HostCapabilityCodegenDecl {
                        host_overrides: &#codegen::WidgetHostOverrideCodegenSet {
                            overrides: &[
                                #(#overrides,)*
                            ],
                        },
                        host_event_mounts: &#codegen::WidgetHostEventMountCodegenSet {
                            mounts: &[
                                #(#event_mounts,)*
                            ],
                        },
                        host_prop_setters: &#codegen::WidgetHostPropSetterCodegenSet {
                            setters: &[
                                #(#prop_setters,)*
                            ],
                        },
                        host_prop_getters: &#codegen::WidgetHostPropGetterCodegenSet {
                            getters: &[
                                #(#prop_getters,)*
                            ],
                        },
                    };
                &DECL
            }
        };
    }
}
