mod model;
mod parse;

use self::model::{PropBehaviorConfig, PropTypeInfo};
pub(crate) use self::parse::struct_declares_widget_props;
use self::parse::{collect_prop_fields, parse_field_config};
use crate::common::{option_inner_type, widget_core_schema_path};
use quote::quote;
use syn::{Data, DeriveInput, Expr, Field, Fields, Ident, Lit, LitStr, Type};

pub(crate) fn should_expand_qt_prop_tree(input: &DeriveInput) -> bool {
    let Data::Struct(data) = &input.data else {
        return false;
    };
    let Fields::Named(fields) = &data.fields else {
        return false;
    };

    fields
        .named
        .iter()
        .any(|field| field.attrs.iter().any(|attr| attr.path().is_ident("qt")))
}

pub(crate) fn expand_qt_prop_tree(input: DeriveInput) -> syn::Result<proc_macro2::TokenStream> {
    let struct_name = input.ident;
    let type_name = LitStr::new(&struct_name.to_string(), struct_name.span());

    let Data::Struct(data) = input.data else {
        return Err(syn::Error::new_spanned(
            struct_name,
            "#[derive(Qt)] prop declarations only support structs",
        ));
    };

    let Fields::Named(fields) = data.fields else {
        return Err(syn::Error::new_spanned(
            struct_name,
            "#[derive(Qt)] prop declarations require named fields",
        ));
    };
    let prop_fields = collect_prop_fields(&fields.named)?;
    if prop_fields.is_empty() {
        return Ok(proc_macro2::TokenStream::new());
    }

    let spec_nodes = expand_spec_prop_nodes(&prop_fields)?;
    let props_dead_code_toucher = expand_props_dead_code_toucher(&struct_name, &prop_fields)?;
    let schema = widget_core_schema_path();

    Ok(quote! {
        impl #schema::QtPropTree for #struct_name {
            fn spec() -> &'static #schema::SpecPropTree {
                static SPEC: #schema::SpecPropTree =
                    #schema::SpecPropTree {
                        type_name: #type_name,
                        nodes: &[
                            #(#spec_nodes,)*
                        ],
                    };

                &SPEC
            }
        }

        #props_dead_code_toucher
    })
}

fn expand_props_dead_code_toucher(
    struct_name: &Ident,
    fields: &[Field],
) -> syn::Result<proc_macro2::TokenStream> {
    let field_names = fields
        .iter()
        .map(|field| {
            field.ident.clone().ok_or_else(|| {
                syn::Error::new_spanned(
                    field,
                    "#[derive(Qt)] prop declarations require named fields",
                )
            })
        })
        .collect::<syn::Result<Vec<_>>>()?;

    Ok(quote! {
        const _: fn(#struct_name) = |value| {
            let #struct_name {
                #(#field_names,)*
            } = value;
            let _ = (
                #(#field_names,)*
            );
        };
    })
}

fn expand_spec_prop_nodes(fields: &[Field]) -> syn::Result<Vec<proc_macro2::TokenStream>> {
    let mut nodes = Vec::new();
    let schema = widget_core_schema_path();

    for field in fields {
        let field_name = field.ident.as_ref().expect("named field");
        let config = parse_field_config(field)?;
        let prop = config
            .prop
            .as_ref()
            .ok_or_else(|| syn::Error::new_spanned(field, "Qt fields require #[qt(prop = ...)]"))?;
        let rust_name = LitStr::new(&prop.rust_name, field_name.span());
        let js_name = LitStr::new(&prop.js_name, field_name.span());
        let prop_type = infer_prop_type_info(&field.ty)?;
        let lowering = meta_lowering(&prop.js_name);
        let read_lowering =
            read_lowering_for_behavior(&config.behavior, meta_lowering(&prop.js_name));
        let value_type = &prop_type.type_tokens;
        let optional = option_inner_type(&field.ty).is_some();
        let behavior = prop_behavior_tokens(config.behavior);
        let exported = config.exported;
        let default = spec_default_value_tokens(&field.ty, config.default.as_ref())?;

        nodes.push(quote! {
            #schema::SpecPropNode::Leaf(#schema::SpecLeafProp {
                rust_name: #rust_name,
                js_name: #js_name,
                value_type: #value_type,
                optional: #optional,
                lowering: #lowering,
                read_lowering: #read_lowering,
                behavior: #behavior,
                exported: #exported,
                default: #default,
            })
        });
    }

    Ok(nodes)
}

fn spec_default_value_tokens(
    ty: &Type,
    default: Option<&Expr>,
) -> syn::Result<proc_macro2::TokenStream> {
    let schema = widget_core_schema_path();
    let value_ty = option_inner_type(ty).unwrap_or(ty);
    let Some(default) = default else {
        return Ok(quote!(#schema::SpecPropDefaultValue::None));
    };
    if option_inner_type(ty).is_some() && matches_default_default(default) {
        return Ok(quote!(#schema::SpecPropDefaultValue::None));
    }

    let value = unwrap_option_default_expr(ty, default)?;
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
                "unsupported #[qt(default = ...)] literal for exported prop",
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
                    "unsupported #[qt(default = ...)] call expression",
                ));
            }
            spec_default_value_tokens(value_ty, call.args.first())
        }
        other => Err(syn::Error::new_spanned(
            other,
            "unsupported #[qt(default = ...)] expression for exported prop",
        )),
    }
}

fn unwrap_option_default_expr<'a>(ty: &Type, default: &'a Expr) -> syn::Result<&'a Expr> {
    if option_inner_type(ty).is_none() {
        return Ok(default);
    }

    let Expr::Call(call) = default else {
        return Err(syn::Error::new_spanned(
            default,
            "Option props require #[qt(default)] or #[qt(default = Some(...))]",
        ));
    };
    let Expr::Path(func) = call.func.as_ref() else {
        return Err(syn::Error::new_spanned(
            call.func.as_ref(),
            "Option prop default expects Some(...)",
        ));
    };
    if !func.path.is_ident("Some") || call.args.len() != 1 {
        return Err(syn::Error::new_spanned(
            call,
            "Option prop default expects Some(...)",
        ));
    }
    call.args
        .first()
        .ok_or_else(|| syn::Error::new_spanned(call, "Option prop default expects Some(...)"))
}

fn matches_default_default(expr: &Expr) -> bool {
    let Expr::Call(call) = expr else {
        return false;
    };
    let Expr::Path(path) = call.func.as_ref() else {
        return false;
    };
    let segments = path.path.segments.iter().collect::<Vec<_>>();
    if segments.len() < 2 {
        return false;
    }
    segments[segments.len() - 2].ident == "Default"
        && segments[segments.len() - 1].ident == "default"
        && call.args.is_empty()
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

fn infer_prop_type_info(ty: &Type) -> syn::Result<PropTypeInfo> {
    let schema = widget_core_schema_path();
    let inner = option_inner_type(ty).unwrap_or(ty);
    Ok(PropTypeInfo {
        type_tokens: quote!(<#inner as #schema::QtType>::INFO),
    })
}

fn meta_lowering(name: &str) -> proc_macro2::TokenStream {
    let schema = widget_core_schema_path();
    quote!(#schema::PropLowering::MetaProperty(#name))
}

fn some_lowering(lowering: proc_macro2::TokenStream) -> proc_macro2::TokenStream {
    quote!(Some(#lowering))
}

fn none_lowering() -> proc_macro2::TokenStream {
    quote!(None)
}

fn read_lowering_for_behavior(
    behavior: &PropBehaviorConfig,
    lowering: proc_macro2::TokenStream,
) -> proc_macro2::TokenStream {
    match behavior {
        PropBehaviorConfig::State => some_lowering(lowering),
        PropBehaviorConfig::Const | PropBehaviorConfig::Command => none_lowering(),
    }
}

fn prop_behavior_tokens(behavior: PropBehaviorConfig) -> proc_macro2::TokenStream {
    let schema = widget_core_schema_path();
    match behavior {
        PropBehaviorConfig::State => quote!(#schema::PropBehavior::State),
        PropBehaviorConfig::Const => quote!(#schema::PropBehavior::Const),
        PropBehaviorConfig::Command => quote!(#schema::PropBehavior::Command),
    }
}
