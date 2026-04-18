use darling::FromMeta;
use syn::LitStr;

pub(super) struct OpaqueDeclConfig {
    pub host: OpaqueHostConfig,
    pub borrow: OpaqueBorrowConfig,
}

#[derive(FromMeta)]
pub(super) struct OpaqueDeclArgs {
    #[darling(default)]
    pub host: Option<OpaqueHostConfig>,
    #[darling(default)]
    pub borrow: Option<LitStr>,
}

#[derive(FromMeta)]
pub(super) struct OpaqueHostConfig {
    pub class: LitStr,
    pub include: LitStr,
}

#[derive(Clone, Copy)]
pub(super) enum OpaqueBorrowConfig {
    Ref,
    Mut,
}
