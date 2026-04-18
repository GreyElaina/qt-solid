use darling::FromMeta;
use syn::Expr;

#[derive(Default)]
pub(super) struct FieldConfig {
    pub prop: Option<FieldPropConfig>,
    pub default: Option<Expr>,
    pub behavior: PropBehaviorConfig,
    pub exported: bool,
}

#[derive(Clone)]
pub(super) struct FieldPropConfig {
    pub rust_name: String,
    pub js_name: String,
}

#[derive(Clone, Copy, Default)]
pub(super) enum PropBehaviorConfig {
    #[default]
    State,
    Const,
    Command,
}

pub(super) struct PropTypeInfo {
    pub type_tokens: proc_macro2::TokenStream,
}

#[derive(Default, FromMeta)]
pub(super) struct FieldAttrArgs {
    #[darling(default)]
    pub prop: Option<FieldPropArg>,
    #[darling(default)]
    pub default: Option<FieldDefaultArg>,
    #[darling(default, rename = "const")]
    pub is_const: bool,
    #[darling(default)]
    pub command: bool,
    #[darling(default)]
    pub export: bool,
}

pub(super) struct FieldPropArg(pub crate::common::FlatPropName);

impl darling::FromMeta for FieldPropArg {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        crate::common::parse_flat_prop_name_expr(expr.clone())
            .map(Self)
            .map_err(|error| darling::Error::custom(error.to_string()).with_span(expr))
    }
}

pub(super) struct FieldDefaultArg(pub Expr);

impl darling::FromMeta for FieldDefaultArg {
    fn from_word() -> darling::Result<Self> {
        Ok(Self(syn::parse_quote!(core::default::Default::default())))
    }

    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        Ok(Self(expr.clone()))
    }
}
