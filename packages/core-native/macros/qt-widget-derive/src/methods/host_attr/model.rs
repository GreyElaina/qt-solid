use darling::FromMeta;
use syn::{Expr, LitStr};

pub(super) struct HostTraitDeclConfig {
    pub host: Option<HostConfig>,
    pub default_layout: Option<Expr>,
}

#[derive(Default, FromMeta)]
pub(super) struct HostTraitDeclArgs {
    #[darling(default)]
    pub class: Option<LitStr>,
    #[darling(default)]
    pub include: Option<LitStr>,
    #[darling(default)]
    pub factory: Option<LitStr>,
    #[darling(default)]
    pub top_level: bool,
    #[darling(default)]
    pub layout: Option<ExprArg>,
}

#[derive(Clone)]
pub(super) struct ExprArg(pub Expr);

impl darling::FromMeta for ExprArg {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        Ok(Self(expr.clone()))
    }
}

#[derive(Clone)]
pub(super) struct NotifyLabelArg(pub String);

impl darling::FromMeta for NotifyLabelArg {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        crate::binding::parse_label_expr(expr.clone())
            .map(Self)
            .map_err(|error| darling::Error::custom(error.to_string()).with_span(expr))
    }
}

#[derive(Clone)]
pub(super) struct HostAttrFlatPropName(pub crate::common::FlatPropName);

impl darling::FromMeta for HostAttrFlatPropName {
    fn from_expr(expr: &Expr) -> darling::Result<Self> {
        crate::common::parse_flat_prop_name_expr(expr.clone())
            .map(Self)
            .map_err(|error| darling::Error::custom(error.to_string()).with_span(expr))
    }
}

#[derive(Default, FromMeta)]
pub(super) struct HostMethodAttrArgs {
    #[darling(default)]
    pub notify: Option<NotifyLabelArg>,
    #[darling(default)]
    pub prop: Option<HostAttrFlatPropName>,
    #[darling(default)]
    pub qt_signal: Option<LitStr>,
    #[darling(default)]
    pub signature: Option<LitStr>,
    #[darling(default)]
    pub default: Option<ExprArg>,
    #[darling(default, rename = "const")]
    pub is_const: bool,
    #[darling(default)]
    pub command: bool,
    #[darling(default)]
    pub setter: bool,
    #[darling(default)]
    pub getter: bool,
}

pub(super) struct HostMethodAttrParts {
    pub args: HostMethodAttrArgs,
    pub exports: Vec<LitStr>,
    pub includes: Vec<LitStr>,
}

#[derive(Clone, Copy)]
pub(super) enum HostDeclHelperKind {
    Spec,
    Codegen,
    Runtime,
}

pub(super) struct HostConfig {
    pub class: LitStr,
    pub include: LitStr,
    pub factory: Option<LitStr>,
    pub top_level: bool,
}

#[derive(Clone)]
pub(super) enum HostCodegenMethodKind {
    PropSetter {
        prop_lower_name: String,
        arg_name: String,
        value_type: proc_macro2::TokenStream,
    },
    PropGetter {
        prop_lower_name: String,
        value_type: proc_macro2::TokenStream,
    },
}

pub(super) struct HostCodegenMethodConfig {
    pub kind: HostCodegenMethodKind,
    pub extra_includes: Vec<LitStr>,
    pub prop: Option<HostPropSpecConfig>,
}

#[derive(Clone)]
pub(super) struct HostPropSpecConfig {
    pub rust_name: String,
    pub js_name: String,
    pub behavior: HostPropBehavior,
    pub default: Option<Expr>,
}

#[derive(Clone, Copy)]
pub(super) enum HostPropBehavior {
    State,
    Const,
    Command,
}

#[derive(Clone)]
pub(super) struct HostPropRecord {
    pub spec: HostPropSpecConfig,
    pub value_type: proc_macro2::TokenStream,
    pub value_type_key: String,
}

pub(super) enum HostTraitMethodConfig {
    Codegen(HostCodegenMethodConfig),
    Signal(HostBehaviorEventConfig),
    Event(HostBehaviorEventConfig),
    Override(HostBehaviorOverrideConfig),
}

pub(super) struct HostBehaviorEventConfig {
    pub label: String,
    pub lower_name: String,
    pub qt_signal_name: Option<String>,
    pub exports: Vec<String>,
    pub echoes: Vec<HostEventEchoConfig>,
    pub extra_includes: Vec<LitStr>,
}

pub(super) struct HostBehaviorOverrideConfig {
    pub signature: String,
    pub extra_includes: Vec<LitStr>,
}

pub(super) struct HostEventEchoConfig {
    pub prop_js_name: String,
    pub value_path: String,
}

pub(super) struct EchoDirectiveArg {
    pub prop_js_name: Option<String>,
}

#[derive(FromMeta)]
pub(super) struct EchoDirectiveBody {
    pub prop: HostAttrFlatPropName,
}

impl darling::FromMeta for EchoDirectiveArg {
    fn from_word() -> darling::Result<Self> {
        Ok(Self { prop_js_name: None })
    }

    fn from_list(items: &[darling::ast::NestedMeta]) -> darling::Result<Self> {
        let body = EchoDirectiveBody::from_list(items)
            .map_err(|_| darling::Error::custom("echo(...) only supports prop = ..."))?;
        Ok(Self {
            prop_js_name: Some(body.prop.0.js_name),
        })
    }
}
