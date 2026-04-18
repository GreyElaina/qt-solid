use crate::schema::{
    EnumMeta, MergedProp, SpecHostMethodMeta, WidgetBinding, all_widget_bindings, merged_props,
    prop_decl, widget_registry,
};
use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use std::collections::BTreeMap;
use syn::{File, Ident, LitStr, Type, parse_str, parse2};

pub fn render_qt_widget_entities_rs() -> String {
    let mut items = render_enum_conversion_impls_tokens(all_widget_bindings());

    for binding in all_widget_bindings() {
        items.extend(render_widget_entity_items(binding));
    }

    render_rs_items(items)
}

pub fn render_qt_node_methods_rs() -> String {
    let mut items = Vec::new();

    for binding in all_widget_bindings() {
        items.push(render_widget_node_methods_impl(binding));
    }

    render_rs_items(items)
}

fn render_rs_items(items: Vec<TokenStream>) -> String {
    let file = parse2::<File>(quote! {
        #(#items)*
    })
    .expect("generated napi bindings should parse as a Rust file");

    prettyplease::unparse(&file)
}

fn prop_decls_for_binding(binding: &WidgetBinding) -> Vec<MergedProp> {
    let spec = widget_registry().spec_binding(binding.spec_key);
    merged_props(spec, prop_decl(binding.spec_key))
}

fn render_widget_entity_items(binding: &WidgetBinding) -> Vec<TokenStream> {
    let entity_name = format_ident!("{}", widget_entity_class_name(binding.kind_name));
    let spec_key = str_lit(binding.spec_key.raw());

    vec![
        quote! {
            #[napi_derive::napi]
            #[derive(Clone)]
            pub struct #entity_name {
                inner: std::sync::Arc<crate::runtime::QtNodeInner>,
            }
        },
        quote! {
            impl #entity_name {
                pub(crate) fn from_inner(inner: std::sync::Arc<crate::runtime::QtNodeInner>) -> Self {
                    Self { inner }
                }
            }
        },
        quote! {
            impl crate::runtime::NodeHandle for #entity_name {
                fn inner(&self) -> &std::sync::Arc<crate::runtime::QtNodeInner> {
                    &self.inner
                }
            }
        },
        quote! {
            #[napi_derive::napi]
            impl #entity_name {
                #[napi(factory)]
                pub fn create(app: &crate::api::QtApp) -> napi::Result<Self> {
                    crate::runtime::create_widget_inner(
                        app.generation(),
                        crate::bootstrap::widget_registry()
                            .binding_by_spec_key_str(#spec_key)
                            .expect("schema widget binding")
                            .widget_type_id,
                    )
                    .map(Self::from_inner)
                }

                #[napi(js_name = "__qtAttach")]
                pub fn __qt_attach(
                    &self,
                    initial_props: Vec<crate::api::QtInitialProp>,
                ) -> napi::Result<()> {
                    crate::runtime::attach_widget_instance(self, initial_props).map(|_| ())
                }

                #[napi(getter)]
                pub fn node(&self) -> crate::api::QtNode {
                    crate::api::QtNode::from_inner(self.inner.clone())
                }

                #[napi(getter)]
                pub fn id(&self) -> u32 {
                    self.inner.id
                }

                #[napi(getter)]
                pub fn parent(&self) -> napi::Result<Option<crate::api::QtNode>> {
                    crate::runtime::node_parent(self)
                }

                #[napi(getter)]
                pub fn first_child(&self) -> napi::Result<Option<crate::api::QtNode>> {
                    crate::runtime::node_first_child(self)
                }

                #[napi(getter)]
                pub fn next_sibling(&self) -> napi::Result<Option<crate::api::QtNode>> {
                    crate::runtime::node_next_sibling(self)
                }

                #[napi]
                pub fn is_text_node(&self) -> bool {
                    crate::runtime::node_is_text_node(self)
                }

                #[napi]
                pub fn insert_child(
                    &self,
                    child: &crate::api::QtNode,
                    anchor: Option<&crate::api::QtNode>,
                ) -> napi::Result<()> {
                    crate::runtime::insert_child(self, child, anchor)
                }

                #[napi]
                pub fn remove_child(&self, child: &crate::api::QtNode) -> napi::Result<()> {
                    crate::runtime::remove_child(self, child)
                }

                #[napi]
                pub fn destroy(&self) -> napi::Result<()> {
                    crate::runtime::destroy_node(self)
                }
            }
        },
    ]
}

fn render_enum_conversion_impls_tokens(bindings: &[&WidgetBinding]) -> Vec<TokenStream> {
    let mut domains = BTreeMap::<&'static str, &'static EnumMeta>::new();

    for binding in bindings {
        for prop in prop_decls_for_binding(binding) {
            if let Some(domain) = prop.value_type.enum_meta() {
                domains.entry(domain.name).or_insert(domain);
            }
        }
        for method in binding.methods.host_methods {
            for arg in method.args {
                if let Some(domain) = arg.value_type.enum_meta() {
                    domains.entry(domain.name).or_insert(domain);
                }
            }
            if let Some(domain) = method.return_type.enum_meta() {
                domains.entry(domain.name).or_insert(domain);
            }
        }
    }

    domains
        .values()
        .flat_map(|domain| {
            let enum_type = rust_type(domain.name);
            let domain_name = str_lit(domain.name);
            let into_arms = render_enum_into_qt_arms(domain);
            let from_arms = render_enum_try_from_qt_arms(domain);

            [
                quote! {
                    impl qt_solid_widget_core::runtime::QtTypeName for #enum_type {
                        fn qt_type_name() -> &'static str {
                            #domain_name
                        }
                    }
                },
                quote! {
                    impl qt_solid_widget_core::runtime::IntoQt for #enum_type {
                        fn into_qt(
                            self,
                        ) -> qt_solid_widget_core::runtime::WidgetResult<
                            qt_solid_widget_core::runtime::QtValue,
                        > {
                            Ok(qt_solid_widget_core::runtime::QtValue::Enum(match self {
                                #(#into_arms)*
                            }))
                        }
                    }
                },
                quote! {
                    impl qt_solid_widget_core::runtime::TryFromQt for #enum_type {
                        fn try_from_qt(
                            value: qt_solid_widget_core::runtime::QtValue,
                        ) -> qt_solid_widget_core::runtime::WidgetResult<Self> {
                            match value {
                                qt_solid_widget_core::runtime::QtValue::Enum(value) => match value {
                                    #(#from_arms)*
                                    value => Err(qt_solid_widget_core::runtime::WidgetError::new(
                                        format!("invalid {} tag {value}", #domain_name),
                                    )),
                                },
                                _ => Err(qt_solid_widget_core::runtime::WidgetError::new(
                                    "expected Qt enum value",
                                )),
                            }
                        }
                    }
                },
            ]
        })
        .collect()
}

fn render_widget_node_methods_impl(binding: &WidgetBinding) -> TokenStream {
    let entity_name = format_ident!("{}", widget_entity_class_name(binding.kind_name));
    let prop_decls = prop_decls_for_binding(binding);
    let prop_methods = prop_decls
        .iter()
        .filter_map(render_widget_prop_setter_method);
    let init_prop_methods = prop_decls
        .iter()
        .filter_map(render_widget_prop_init_setter_method);
    let getter_methods = prop_decls
        .iter()
        .filter_map(render_widget_prop_getter_method);
    let host_methods = binding
        .methods
        .host_methods
        .iter()
        .map(|method| render_widget_method(binding, method));

    quote! {
        #[napi_derive::napi]
        impl #entity_name {
            #(#prop_methods)*
            #(#init_prop_methods)*
            #(#getter_methods)*
            #(#host_methods)*
        }
    }
}

fn render_widget_prop_setter_method(prop: &MergedProp) -> Option<TokenStream> {
    let js_method = setter_method_name(prop)?;
    let js_name = str_lit(&js_method);
    let rust_method = format_ident!("set_{}", snake_case(&js_method[3..]));
    let rust_type = rust_type(prop.value_type.rust_path());
    let body = if let Some(slot) = prop.write_slot().filter(|_| prop.has_live_write()) {
        quote! {
            crate::runtime::ensure_live_node(self)?;
            let instance = crate::runtime::widget_instance_for_node_id(self.inner().id)?;
            instance
                .apply_prop(#slot, value)
                .map_err(|error| crate::runtime::qt_error(error.to_string()))
        }
    } else if let Some(slot) = prop.init_only_slot() {
        quote! {
            crate::runtime::ensure_live_node(self)?;
            let instance = crate::runtime::widget_instance_for_node_id(self.inner().id)?;
            instance
                .apply_prop(#slot, value)
                .map_err(|error| crate::runtime::qt_error(error.to_string()))
        }
    } else {
        let prop_name = str_lit(&prop.key);
        quote! {
            crate::runtime::apply_qt_prop_by_name(self, #prop_name, value)
        }
    };

    Some(quote! {
        #[napi(js_name = #js_name)]
        pub fn #rust_method(&self, value: #rust_type) -> napi::Result<()> {
            let value = <#rust_type as qt_solid_widget_core::runtime::IntoQt>::into_qt(value)
                .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
            #body
        }
    })
}

fn render_widget_prop_init_setter_method(prop: &MergedProp) -> Option<TokenStream> {
    let slot = prop.init_only_slot()?;
    let js_method = init_setter_method_name(prop)?;
    let public_js_method = setter_method_name(prop);
    if public_js_method.as_deref() == Some(js_method.as_str()) {
        return None;
    }

    let js_name = str_lit(&js_method);
    let rust_method = format_ident!("{}", snake_case(&js_method));
    let rust_type = rust_type(prop.value_type.rust_path());

    Some(quote! {
        #[napi(js_name = #js_name)]
        pub fn #rust_method(&self, value: #rust_type) -> napi::Result<()> {
            let value = <#rust_type as qt_solid_widget_core::runtime::IntoQt>::into_qt(value)
                .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
            crate::runtime::ensure_live_node(self)?;
            let instance = crate::runtime::widget_instance_for_node_id(self.inner().id)?;
            instance
                .apply_prop(#slot, value)
                .map_err(|error| crate::runtime::qt_error(error.to_string()))
        }
    })
}

fn render_widget_prop_getter_method(prop: &MergedProp) -> Option<TokenStream> {
    let slot = prop.read_slot()?;
    let js_method = getter_method_name(prop);
    let js_name = str_lit(&js_method);
    let rust_method = format_ident!("get_{}", snake_case(&js_method[3..]));
    let rust_type = rust_type(prop.value_type.rust_path());

    Some(quote! {
        #[napi(js_name = #js_name)]
        pub fn #rust_method(&self) -> napi::Result<#rust_type> {
            crate::runtime::ensure_live_node(self)?;
            let instance = crate::runtime::widget_instance_for_node_id(self.inner().id)?;
            let value = instance
                .read_prop(#slot)
                .map_err(|error| crate::runtime::qt_error(error.to_string()))?;
            <#rust_type as qt_solid_widget_core::runtime::TryFromQt>::try_from_qt(value)
                .map_err(|error| crate::runtime::qt_error(error.to_string()))
        }
    })
}

fn render_widget_method(_binding: &WidgetBinding, method: &SpecHostMethodMeta) -> TokenStream {
    let js_name = str_lit(method.js_name);
    let rust_method = format_ident!("{}", snake_case(method.rust_name));
    let rust_return_type = rust_type(method.return_type.rust_path());
    let signature_args = method.args.iter().map(render_method_arg_signature);
    let args_value = method.args.iter().map(render_method_arg_value);
    let slot = method.slot;

    quote! {
        #[napi(js_name = #js_name)]
        pub fn #rust_method(&self #(, #signature_args)*) -> napi::Result<#rust_return_type> {
            let value = crate::runtime::call_host_method_slot(self, #slot, vec![#(#args_value),*])?;
            <#rust_return_type as qt_solid_widget_core::runtime::TryFromQt>::try_from_qt(value)
                .map_err(|error| crate::runtime::qt_error(error.to_string()))
        }
    }
}

fn render_method_arg_signature(arg: &crate::schema::SpecHostMethodArg) -> TokenStream {
    let arg_name = rust_ident(arg.rust_name);
    let arg_type = rust_type(arg.value_type.rust_path());

    quote! {
        #arg_name: #arg_type
    }
}

fn render_method_arg_value(arg: &crate::schema::SpecHostMethodArg) -> TokenStream {
    let arg_name = rust_ident(arg.rust_name);

    quote! {
        {
            qt_solid_widget_core::runtime::IntoQt::into_qt(#arg_name)
                .map_err(|error| crate::runtime::qt_error(error.to_string()))?
        }
    }
}

fn render_enum_into_qt_arms(domain: &EnumMeta) -> Vec<TokenStream> {
    let enum_type = rust_type(domain.name);

    domain
        .values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let variant = format_ident!("{}", pascal_case(value));
            let tag = i32::try_from(index + 1).expect("enum tag fits in i32");

            quote! {
                #enum_type::#variant => #tag,
            }
        })
        .collect()
}

fn render_enum_try_from_qt_arms(domain: &EnumMeta) -> Vec<TokenStream> {
    let enum_type = rust_type(domain.name);

    domain
        .values
        .iter()
        .enumerate()
        .map(|(index, value)| {
            let variant = format_ident!("{}", pascal_case(value));
            let tag = i32::try_from(index + 1).expect("enum tag fits in i32");

            quote! {
                #tag => Ok(#enum_type::#variant),
            }
        })
        .collect()
}

fn rust_ident(value: &str) -> Ident {
    parse_str(value).unwrap_or_else(|error| panic!("invalid Rust ident {value}: {error}"))
}

fn rust_type(value: &str) -> Type {
    parse_str(value).unwrap_or_else(|error| panic!("invalid Rust type {value}: {error}"))
}

fn str_lit(value: &str) -> LitStr {
    LitStr::new(value, Span::call_site())
}

fn widget_entity_class_name(kind_name: &str) -> String {
    format!("Qt{}", pascal_case(kind_name))
}

fn path_method_suffix<'a>(path: impl IntoIterator<Item = &'a str>) -> String {
    path.into_iter().map(pascal_case).collect::<String>()
}

fn setter_method_name(prop: &MergedProp) -> Option<String> {
    if matches!(
        prop.write_mode(),
        qt_solid_widget_core::schema::EndpointWriteMode::None
    ) {
        return None;
    }

    Some(format!(
        "set{}",
        path_method_suffix(prop.path.iter().copied())
    ))
}

fn init_setter_method_name(prop: &MergedProp) -> Option<String> {
    prop.init_only_slot()?;

    if !prop.has_live_write() {
        return setter_method_name(prop);
    }

    Some(format!(
        "__qtInit{}",
        path_method_suffix(prop.path.iter().copied())
    ))
}

fn getter_method_name(prop: &MergedProp) -> String {
    format!("get{}", path_method_suffix(prop.path.iter().copied()))
}

fn pascal_case(value: &str) -> String {
    value
        .split(['_', '-'])
        .filter(|segment| !segment.is_empty())
        .map(|segment| {
            let mut chars = segment.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().chain(chars).collect::<String>(),
                None => String::new(),
            }
        })
        .collect::<String>()
}

fn snake_case(value: &str) -> String {
    let mut out = String::new();
    for (index, ch) in value.chars().enumerate() {
        if ch.is_uppercase() {
            if index > 0 {
                out.push('_');
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
    use super::*;

    #[test]
    fn widget_entities_render_known_binding_names() {
        let output = render_qt_widget_entities_rs();
        let binding = all_widget_bindings()
            .first()
            .expect("expected at least one widget binding");
        let entity_name = widget_entity_class_name(binding.kind_name);

        assert!(output.contains(&format!("pub struct {entity_name}")));
        assert!(output.contains("impl crate::runtime::NodeHandle"));
    }

    #[test]
    fn node_methods_render_known_binding_impls() {
        let output = render_qt_node_methods_rs();
        let binding = all_widget_bindings()
            .first()
            .expect("expected at least one widget binding");
        let entity_name = widget_entity_class_name(binding.kind_name);

        assert!(output.contains(&format!("impl {entity_name}")));
        assert!(output.contains("#[napi(js_name ="));
    }
}
