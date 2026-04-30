use proc_macro2::TokenStream;
use quote::quote;
use syn::DeriveInput;

use crate::parse::{BoundsKind, FieldMode, FragmentFieldAttrs, FragmentStructAttrs};

#[derive(Clone, Copy)]
enum DirectVariant { F64, Str, Bool }

#[derive(Clone, PartialEq)]
enum ParseMode { Direct, Color, StrokeColor, PlainColor, Brush, Shadow, Radii, Border }

#[derive(Clone)]
enum ClearMode { Default, None_ }

struct PropField {
    rust_name: syn::Ident,
    js_name: String,
    direct_variant: Option<DirectVariant>,
    parse_mode: ParseMode,
    clear_mode: ClearMode,
    default_expr: Option<syn::Expr>,
    mutation_flags: TokenStream,
}

pub fn expand(input: DeriveInput) -> syn::Result<TokenStream> {
    let struct_attrs = FragmentStructAttrs::from_attrs(&input.attrs)?;
    let ident = &input.ident;

    let fields = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Named(named) => &named.named,
            _ => {
                return Err(syn::Error::new_spanned(
                    ident,
                    "#[derive(Fragment)] only supports structs with named fields",
                ));
            }
        },
        _ => unreachable!(),
    };

    let mut prop_fields = Vec::new();

    for field in fields {
        let field_attrs = FragmentFieldAttrs::from_field(field)?;
        if field_attrs.mode == FieldMode::Skip {
            continue;
        }

        let field_ident = field.ident.as_ref().unwrap();

        let parse_mode = match field_attrs.parse.as_deref() {
            Some("color") => ParseMode::Color,
            Some("stroke_color") => ParseMode::StrokeColor,
            Some("plain_color") => ParseMode::PlainColor,
            Some("brush") => ParseMode::Brush,
            Some("shadow") => ParseMode::Shadow,
            Some("radii") => ParseMode::Radii,
            Some("border") => ParseMode::Border,
            _ => ParseMode::Direct,
        };

        let direct_variant = if parse_mode == ParseMode::Direct {
            Some(infer_direct_variant(&field.ty)?)
        } else {
            None
        };

        let clear_mode = match field_attrs.clear.as_deref() {
            Some("none") => ClearMode::None_,
            _ => ClearMode::Default,
        };
        let mutation_flags = match field_attrs.mutation.as_deref() {
            Some("paint") => quote! { FragmentMutation::PAINT },
            Some("layout") => quote! { FragmentMutation::LAYOUT },
            Some("hit_test") => quote! { FragmentMutation::HIT_TEST },
            Some("reshape_text") => quote! { FragmentMutation::RESHAPE_TEXT },
            _ => quote! { FragmentMutation::PAINT },
        };

        let js_name = field_attrs
            .js_name
            .unwrap_or_else(|| field_ident.to_string());

        prop_fields.push(PropField {
            rust_name: field_ident.clone(),
            js_name,
            direct_variant,
            parse_mode,
            clear_mode,
            default_expr: field_attrs.default_expr,
            mutation_flags,
        });
    }

    // Generate apply_prop match arms.
    let apply_arms: Vec<TokenStream> = prop_fields.iter().map(|pf| {
        let js = &pf.js_name;
        let rust = &pf.rust_name;
        let mutation = &pf.mutation_flags;

        match &pf.parse_mode {
            ParseMode::Color => {
                quote! {
                    #js => {
                        if let Some(color) = crate::fragment::parse_color_from_wire(&value) {
                            self.#rust = Some(crate::fragment::FillPaint {
                                color,
                                rule: crate::vello::peniko::Fill::NonZero,
                            });
                            return #mutation;
                        }
                        FragmentMutation::NONE
                    }
                }
            }
            ParseMode::StrokeColor => {
                quote! {
                    #js => {
                        if let Some(color) = crate::fragment::parse_color_from_wire(&value) {
                            let w = self.#rust.as_ref().map_or(1.0, |s| s.width);
                            self.#rust = Some(crate::fragment::StrokePaint { color, width: w });
                            return #mutation;
                        }
                        FragmentMutation::NONE
                    }
                }
            }
            ParseMode::PlainColor => {
                quote! {
                    #js => {
                        if let Some(color) = crate::fragment::parse_color_from_wire(&value) {
                            self.#rust = color;
                            return #mutation;
                        }
                        FragmentMutation::NONE
                    }
                }
            }
            ParseMode::Direct => {
                match pf.direct_variant.unwrap() {
                    DirectVariant::F64 => {
                        quote! {
                            #js => {
                                if let FragmentValue::F64 { value } = value {
                                    self.#rust = value;
                                    return #mutation;
                                }
                                FragmentMutation::NONE
                            }
                        }
                    }
                    DirectVariant::Str => {
                        quote! {
                            #js => {
                                if let FragmentValue::Str { value } = value {
                                    self.#rust = value;
                                    return #mutation;
                                }
                                FragmentMutation::NONE
                            }
                        }
                    }
                    DirectVariant::Bool => {
                        quote! {
                            #js => {
                                if let FragmentValue::Bool { value } = value {
                                    self.#rust = value;
                                    return #mutation;
                                }
                                FragmentMutation::NONE
                            }
                        }
                    }
                }
            }
            ParseMode::Brush => {
                quote! {
                    #js => {
                        if let Some(brush) = crate::fragment::parse_brush_from_wire(&value) {
                            self.#rust = Some(brush);
                            return #mutation;
                        }
                        FragmentMutation::NONE
                    }
                }
            }
            ParseMode::Shadow => {
                quote! {
                    #js => {
                        if let Some(shadow) = crate::fragment::parse_shadow_from_wire(&value) {
                            self.#rust = Some(shadow);
                            return #mutation;
                        }
                        FragmentMutation::NONE
                    }
                }
            }
            ParseMode::Radii => {
                quote! {
                    #js => {
                        if let Some(radii) = crate::fragment::parse_radii_from_wire(&value) {
                            self.#rust = radii;
                            return #mutation;
                        }
                        FragmentMutation::NONE
                    }
                }
            }
            ParseMode::Border => {
                quote! {
                    #js => {
                        if let Some(border) = crate::fragment::parse_border_from_wire(&value) {
                            self.#rust = Some(border);
                            return #mutation;
                        }
                        FragmentMutation::NONE
                    }
                }
            }
        }
    }).collect();

    // Generate reset_prop match arms.
    let reset_arms: Vec<TokenStream> = prop_fields.iter().map(|pf| {
        let js = &pf.js_name;
        let rust = &pf.rust_name;
        let mutation = &pf.mutation_flags;

        match &pf.clear_mode {
            ClearMode::None_ => {
                quote! {
                    #js => {
                        self.#rust = None;
                        #mutation
                    }
                }
            }
            ClearMode::Default => {
                if let Some(expr) = &pf.default_expr {
                    quote! {
                        #js => {
                            self.#rust = #expr;
                            #mutation
                        }
                    }
                } else {
                    quote! {
                        #js => {
                            self.#rust = Default::default();
                            #mutation
                        }
                    }
                }
            }
        }
    }).collect();

    // Generate PROPS const.
    let prop_decls: Vec<TokenStream> = prop_fields.iter().map(|pf| {
        let rust_name = pf.rust_name.to_string();
        let js_name = &pf.js_name;
        let mutation = &pf.mutation_flags;
        quote! {
            FragmentPropDecl {
                rust_name: #rust_name,
                js_name: #js_name,
                mutation: #mutation,
            }
        }
    }).collect();

    let tag = &struct_attrs.tag;

    // Generate local_bounds.
    let local_bounds_body = match struct_attrs.bounds {
        BoundsKind::Rect => quote! {
            Some(crate::vello::peniko::kurbo::Rect::new(0.0, 0.0, self.width, self.height))
        },
        BoundsKind::Circle => quote! {
            Some(crate::vello::peniko::kurbo::Rect::new(
                0.0, 0.0,
                self.r * 2.0, self.r * 2.0,
            ))
        },
        BoundsKind::Text => quote! {
            self.shaped.as_ref().map(|s| {
                crate::vello::peniko::kurbo::Rect::new(0.0, 0.0, s.width, s.height)
            })
        },
        BoundsKind::TextInput => quote! {
            self.layout.as_ref().map(|s| {
                crate::vello::peniko::kurbo::Rect::new(0.0, 0.0, s.width, s.height)
            })
        },
        BoundsKind::None => quote! { None },
    };

    let expanded = quote! {
        impl crate::fragment::decl::FragmentDecl for #ident {
            const TAG: &'static str = #tag;
            const PROPS: &'static [crate::fragment::decl::FragmentPropDecl] = &[
                #(#prop_decls),*
            ];

            fn apply_prop(
                &mut self,
                key: &str,
                value: crate::fragment::decl::FragmentValue,
            ) -> crate::fragment::decl::FragmentMutation {
                use crate::fragment::decl::{FragmentValue, FragmentMutation, FragmentPropDecl};
                match key {
                    #(#apply_arms)*
                    _ => FragmentMutation::NONE,
                }
            }

            fn reset_prop(
                &mut self,
                key: &str,
            ) -> crate::fragment::decl::FragmentMutation {
                use crate::fragment::decl::FragmentMutation;
                match key {
                    #(#reset_arms)*
                    _ => FragmentMutation::NONE,
                }
            }

            fn local_bounds(&self) -> Option<crate::vello::peniko::kurbo::Rect> {
                #local_bounds_body
            }
        }
    };

    Ok(expanded)
}

fn infer_direct_variant(ty: &syn::Type) -> syn::Result<DirectVariant> {
    let type_str = quote::quote!(#ty).to_string().replace(' ', "");
    match type_str.as_str() {
        "f64" | "f32" => Ok(DirectVariant::F64),
        "String" => Ok(DirectVariant::Str),
        "bool" => Ok(DirectVariant::Bool),
        _ => Err(syn::Error::new_spanned(
            ty,
            format!("cannot infer direct variant for `{type_str}`; add #[fragment(prop, parse = ...)]"),
        )),
    }
}
