use darling::FromMeta;
use syn::{Ident, LitStr, Path};

pub(super) struct WidgetFieldDefault {
    pub ident: Ident,
    pub default: Option<proc_macro2::TokenStream>,
}

pub(super) struct WidgetDeclConfig {
    pub export: Option<LitStr>,
    pub children: Ident,
    pub host: Option<HostConfig>,
}

#[derive(FromMeta)]
pub(super) struct HostConfig {
    pub class: LitStr,
    pub include: LitStr,
    #[darling(default)]
    pub factory: Option<LitStr>,
    #[darling(default)]
    pub top_level: bool,
}

#[derive(FromMeta)]
pub(super) struct WidgetDeclArgs {
    #[darling(default)]
    pub export: Option<LitStr>,
    #[darling(default)]
    pub children: Option<Path>,
    #[darling(default)]
    pub host: Option<HostConfig>,
}
