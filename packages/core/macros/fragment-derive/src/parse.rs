use syn::{Attribute, Expr, Lit, Meta, Token};
use syn::punctuated::Punctuated;

// ---------------------------------------------------------------------------
// Struct-level: #[fragment(tag = "rect", bounds = rect)]
// ---------------------------------------------------------------------------

pub struct FragmentStructAttrs {
    pub tag: String,
    pub bounds: BoundsKind,
}

#[derive(Debug, Clone, Copy)]
pub enum BoundsKind {
    None,
    Rect,
    Circle,
    Text,
    TextInput,
}

impl FragmentStructAttrs {
    pub fn from_attrs(attrs: &[Attribute]) -> syn::Result<Self> {
        let mut tag = None;
        let mut bounds = BoundsKind::None;

        for attr in attrs {
            if !attr.path().is_ident("fragment") {
                continue;
            }
            let nested: Punctuated<Meta, Token![,]> =
                attr.parse_args_with(Punctuated::parse_terminated)?;
            for meta in &nested {
                match meta {
                    Meta::NameValue(nv) if nv.path.is_ident("tag") => {
                        if let Expr::Lit(lit) = &nv.value {
                            if let Lit::Str(s) = &lit.lit {
                                tag = Some(s.value());
                            }
                        }
                    }
                    Meta::NameValue(nv) if nv.path.is_ident("bounds") => {
                        if let Expr::Path(path) = &nv.value {
                            if let Some(ident) = path.path.get_ident() {
                                bounds = match ident.to_string().as_str() {
                                    "rect" => BoundsKind::Rect,
                                    "circle" => BoundsKind::Circle,
                                    "text" => BoundsKind::Text,
                                    "text_input" => BoundsKind::TextInput,
                                    "none" => BoundsKind::None,
                                    _ => {
                                        return Err(syn::Error::new_spanned(
                                            ident,
                                            "expected one of: rect, circle, text, text_input, none",
                                        ));
                                    }
                                };
                            }
                        }
                    }
                    other => {
                        return Err(syn::Error::new_spanned(
                            other,
                            "unknown fragment struct attribute",
                        ));
                    }
                }
            }
        }

        let tag = tag.ok_or_else(|| {
            syn::Error::new(proc_macro2::Span::call_site(), "missing #[fragment(tag = \"...\")]")
        })?;

        Ok(Self { tag, bounds })
    }
}

// ---------------------------------------------------------------------------
// Field-level: #[fragment(prop)], #[fragment(prop(js = "cornerRadius"))],
//              #[fragment(skip)], #[fragment(prop, parse = color, clear = none)]
// ---------------------------------------------------------------------------

pub struct FragmentFieldAttrs {
    pub mode: FieldMode,
    pub js_name: Option<String>,
    pub parse: Option<String>,
    pub clear: Option<String>,
    pub default_expr: Option<Expr>,
    pub mutation: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldMode {
    Prop,
    Skip,
}

impl FragmentFieldAttrs {
    pub fn from_field(field: &syn::Field) -> syn::Result<Self> {
        let mut mode = FieldMode::Prop;
        let mut js_name = None;
        let mut parse = None;
        let mut clear = None;
        let mut default_expr = None;
        let mut mutation = None;
        let mut has_fragment_attr = false;

        for attr in &field.attrs {
            if !attr.path().is_ident("fragment") {
                continue;
            }
            has_fragment_attr = true;

            let nested: Punctuated<Meta, Token![,]> =
                attr.parse_args_with(Punctuated::parse_terminated)?;

            for meta in &nested {
                match meta {
                    Meta::Path(path) if path.is_ident("prop") => {
                        mode = FieldMode::Prop;
                    }
                    Meta::Path(path) if path.is_ident("skip") => {
                        mode = FieldMode::Skip;
                    }
                    Meta::List(list) if list.path.is_ident("prop") => {
                        mode = FieldMode::Prop;
                        let inner: Punctuated<Meta, Token![,]> =
                            list.parse_args_with(Punctuated::parse_terminated)?;
                        for imeta in &inner {
                            if let Meta::NameValue(nv) = imeta {
                                if nv.path.is_ident("js") {
                                    if let Expr::Lit(lit) = &nv.value {
                                        if let Lit::Str(s) = &lit.lit {
                                            js_name = Some(s.value());
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Meta::NameValue(nv) if nv.path.is_ident("default") => {
                        default_expr = Some(nv.value.clone());
                    }
                    Meta::NameValue(nv) if nv.path.is_ident("parse") => {
                        if let Expr::Path(path) = &nv.value {
                            if let Some(ident) = path.path.get_ident() {
                                parse = Some(ident.to_string());
                            }
                        }
                    }
                    Meta::NameValue(nv) if nv.path.is_ident("clear") => {
                        if let Expr::Path(path) = &nv.value {
                            if let Some(ident) = path.path.get_ident() {
                                clear = Some(ident.to_string());
                            }
                        }
                    }
                    Meta::NameValue(nv) if nv.path.is_ident("mutation") => {
                        if let Expr::Path(path) = &nv.value {
                            if let Some(ident) = path.path.get_ident() {
                                mutation = Some(ident.to_string());
                            }
                        }
                    }
                    _ => {}
                }
            }
        }

        if !has_fragment_attr {
            mode = FieldMode::Skip;
        }

        Ok(Self {
            mode,
            js_name,
            parse,
            clear,
            default_expr,
            mutation,
        })
    }
}
