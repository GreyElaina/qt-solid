mod model;
mod parse;

use self::model::OpaqueBorrowConfig;
use self::parse::{parse_qt_opaque_config, reject_qt_field_attrs, validate_opaque_generics};
use crate::common::{
    sanitize_cpp_symbol_segment, widget_core_codegen_path, widget_core_runtime_path,
    widget_core_schema_path,
};
use quote::{format_ident, quote};
use syn::{Fields, ItemStruct, LitStr, Token, punctuated::Punctuated};

pub(crate) fn expand_qt_opaque_attr(
    meta_items: Punctuated<syn::Meta, Token![,]>,
    input: ItemStruct,
) -> syn::Result<proc_macro2::TokenStream> {
    let codegen = widget_core_codegen_path();
    let config = parse_qt_opaque_config(meta_items)?;
    let schema = widget_core_schema_path();
    let runtime = widget_core_runtime_path();
    let struct_name = input.ident.clone();
    let inner_name = format_ident!("__{struct_name}Inner");
    let attrs = input.attrs.clone();
    let visibility = input.vis.clone();
    let generics = input.generics.clone();
    let lifetime = validate_opaque_generics(&generics)?;

    let Fields::Named(fields) = input.fields else {
        return Err(syn::Error::new_spanned(
            struct_name,
            "#[qt_entity(opaque, ...)] requires a named-field struct",
        ));
    };
    reject_qt_field_attrs(&fields)?;

    if fields.named.iter().any(|field| {
        field
            .ident
            .as_ref()
            .is_some_and(|ident| ident == "__qt_inner")
    }) {
        return Err(syn::Error::new_spanned(
            struct_name,
            "#[qt_entity(opaque, ...)] reserves the field name __qt_inner",
        ));
    }

    let borrow_kind = match config.borrow {
        OpaqueBorrowConfig::Ref => quote!(#runtime::QtOpaqueBorrow::Ref),
        OpaqueBorrowConfig::Mut => quote!(#runtime::QtOpaqueBorrow::Mut),
    };
    let class = config.host.class.clone();
    let include = config.host.include.clone();
    let bridge_suffix = sanitize_cpp_symbol_segment(&class.value());
    let host_call_fn = LitStr::new(&format!("qt_{bridge_suffix}_call"), class.span());
    let hook_bridge_fn = LitStr::new(&format!("qt_invoke_{bridge_suffix}_hook"), class.span());
    let host_field_ty = match config.borrow {
        OpaqueBorrowConfig::Ref => quote!(&#lifetime dyn #runtime::QtOpaqueHostRefDyn),
        OpaqueBorrowConfig::Mut => quote!(&#lifetime mut dyn #runtime::QtOpaqueHostMutDyn),
    };
    let user_fields = fields.named.iter().cloned().collect::<Vec<_>>();
    let default_fields = fields
        .named
        .iter()
        .map(|field| {
            let ident = field.ident.as_ref().expect("named field");
            quote!(#ident: core::default::Default::default())
        })
        .collect::<Vec<_>>();
    let (impl_generics, ty_generics, where_clause) = generics.split_for_impl();

    let from_host_impl = match config.borrow {
        OpaqueBorrowConfig::Ref => quote! {
            pub fn __qt_from_host(
                host: &#lifetime dyn #runtime::QtOpaqueHostRefDyn,
            ) -> #runtime::WidgetResult<Self> {
                let expected = <Self as #runtime::QtOpaqueFacade>::INFO;
                let actual = host.opaque_info();
                if !expected.matches_host(actual) {
                    return Err(#runtime::WidgetError::new(format!(
                        "expected opaque host {} from {} with {:?} borrow, got {} from {} with {:?} borrow",
                        expected.cxx_class(),
                        expected.cxx_include(),
                        expected.borrow(),
                        actual.cxx_class(),
                        actual.cxx_include(),
                        actual.borrow()
                    )));
                }

                Ok(Self {
                    __qt_inner: #inner_name { __qt_host: host },
                    #(#default_fields,)*
                })
            }
        },
        OpaqueBorrowConfig::Mut => quote! {
            pub fn __qt_from_host(
                host: &#lifetime mut dyn #runtime::QtOpaqueHostMutDyn,
            ) -> #runtime::WidgetResult<Self> {
                let expected = <Self as #runtime::QtOpaqueFacade>::INFO;
                let actual = host.opaque_info();
                if !expected.matches_host(actual) {
                    return Err(#runtime::WidgetError::new(format!(
                        "expected opaque host {} from {} with {:?} borrow, got {} from {} with {:?} borrow",
                        expected.cxx_class(),
                        expected.cxx_include(),
                        expected.borrow(),
                        actual.cxx_class(),
                        actual.cxx_include(),
                        actual.borrow()
                    )));
                }

                Ok(Self {
                    __qt_inner: #inner_name { __qt_host: host },
                    #(#default_fields,)*
                })
            }
        },
    };

    let inner_host_owner_impl = match config.borrow {
        OpaqueBorrowConfig::Ref => quote! {
            impl #impl_generics #runtime::QtHostMethodOwner for #inner_name #ty_generics #where_clause {
                fn __qt_call_host_method(
                    &self,
                    slot: u16,
                    name: &str,
                    args: std::vec::Vec<#runtime::QtValue>,
                ) -> #runtime::WidgetResult<#runtime::QtValue> {
                    let _ = name;
                    self.__qt_host.call_host_slot(slot, &args)
                }
            }
        },
        OpaqueBorrowConfig::Mut => quote! {
            impl #impl_generics #runtime::QtHostMethodOwnerMut for #inner_name #ty_generics #where_clause {
                fn __qt_call_host_method_mut(
                    &mut self,
                    slot: u16,
                    name: &str,
                    args: std::vec::Vec<#runtime::QtValue>,
                ) -> #runtime::WidgetResult<#runtime::QtValue> {
                    let _ = name;
                    self.__qt_host.call_host_slot_mut(slot, &args)
                }
            }
        },
    };

    let outer_host_owner_impl = match config.borrow {
        OpaqueBorrowConfig::Ref => quote! {
            impl #impl_generics #runtime::QtHostMethodOwner for #struct_name #ty_generics #where_clause {
                fn __qt_call_host_method(
                    &self,
                    slot: u16,
                    name: &str,
                    args: std::vec::Vec<#runtime::QtValue>,
                ) -> #runtime::WidgetResult<#runtime::QtValue> {
                    <#inner_name #ty_generics as #runtime::QtHostMethodOwner>::__qt_call_host_method(
                        &self.__qt_inner,
                        slot,
                        name,
                        args,
                    )
                }
            }
        },
        OpaqueBorrowConfig::Mut => quote! {
            impl #impl_generics #runtime::QtHostMethodOwnerMut for #struct_name #ty_generics #where_clause {
                fn __qt_call_host_method_mut(
                    &mut self,
                    slot: u16,
                    name: &str,
                    args: std::vec::Vec<#runtime::QtValue>,
                ) -> #runtime::WidgetResult<#runtime::QtValue> {
                    <#inner_name #ty_generics as #runtime::QtHostMethodOwnerMut>::__qt_call_host_method_mut(
                        &mut self.__qt_inner,
                        slot,
                        name,
                        args,
                    )
                }
            }
        },
    };

    Ok(quote! {
        #(#attrs)*
        #visibility struct #struct_name #generics {
            #[doc(hidden)]
            __qt_inner: #inner_name #ty_generics,
            #(#user_fields,)*
        }

        #[doc(hidden)]
        #visibility struct #inner_name #generics {
            #[doc(hidden)]
            __qt_host: #host_field_ty,
        }

        impl #impl_generics #struct_name #ty_generics #where_clause {
            #from_host_impl
        }

        impl #impl_generics #runtime::QtOpaqueFacade for #struct_name #ty_generics #where_clause {
            const INFO: #runtime::QtOpaqueInfo = #runtime::QtOpaqueInfo::new(
                concat!(module_path!(), "::", stringify!(#struct_name)),
                #class,
                #include,
                #borrow_kind,
            );
        }

        impl #impl_generics #schema::QtOpaqueDecl for #struct_name #ty_generics #where_clause {
            const SPEC: #schema::SpecOpaqueDecl = #schema::SpecOpaqueDecl {
                opaque: <Self as #runtime::QtOpaqueFacade>::INFO,
                methods: &<Self as #schema::QtHostMethodSurface>::SPEC,
            };
        }

        impl #impl_generics #codegen::QtOpaqueCodegenDecl for #struct_name #ty_generics #where_clause {
            const CODEGEN: #codegen::OpaqueCodegenDecl = #codegen::OpaqueCodegenDecl {
                opaque: <Self as #runtime::QtOpaqueFacade>::INFO,
                methods: &<Self as #codegen::QtOpaqueMethodCodegenSurface>::CODEGEN,
                host_call_fn: <Self as #codegen::QtOpaqueCodegenBridge>::HOST_CALL_FN,
                hook_bridge_fn: <Self as #codegen::QtOpaqueCodegenBridge>::HOOK_BRIDGE_FN,
            };
        }

        impl #impl_generics #codegen::QtOpaqueCodegenBridge for #struct_name #ty_generics #where_clause {
            const HOST_CALL_FN: &'static str = #host_call_fn;
            const HOOK_BRIDGE_FN: &'static str = #hook_bridge_fn;
        }

        #inner_host_owner_impl
        #outer_host_owner_impl
    })
}
