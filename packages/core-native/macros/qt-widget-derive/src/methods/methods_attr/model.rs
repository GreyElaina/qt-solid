use syn::LitStr;

#[derive(Clone)]
pub(super) struct OpaqueCodegenLoweringConfig {
    pub extra_includes: Vec<LitStr>,
    pub body: LitStr,
}

pub(super) struct MethodConfig {
    pub lowering: MethodLoweringConfig,
    pub extra_includes: Vec<LitStr>,
}

#[derive(Clone)]
pub(super) enum MethodLoweringConfig {
    Plain,
    Host { host_name_override: Option<String> },
}

#[derive(Clone, Copy)]
pub(super) enum HostMethodReceiverKind {
    Shared,
    Mutable,
}
