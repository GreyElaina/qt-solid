use crate::{decl::SpecWidgetKey, runtime::QtOpaqueInfo};

pub use linkme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpaqueCodegenLowering {
    pub extra_includes: &'static [&'static str],
    pub body: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpaqueMethodCodegenMeta {
    pub slot: u16,
    pub lowering: Option<OpaqueCodegenLowering>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpaqueMethodCodegenSet {
    pub methods: &'static [OpaqueMethodCodegenMeta],
}

pub const NO_OPAQUE_METHOD_CODEGEN: OpaqueMethodCodegenSet =
    OpaqueMethodCodegenSet { methods: &[] };

pub trait QtOpaqueMethodCodegenSurface {
    const CODEGEN: OpaqueMethodCodegenSet;
}

pub struct NoOpaqueMethodCodegen;

impl QtOpaqueMethodCodegenSurface for NoOpaqueMethodCodegen {
    const CODEGEN: OpaqueMethodCodegenSet = NO_OPAQUE_METHOD_CODEGEN;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OpaqueCodegenDecl {
    pub opaque: QtOpaqueInfo,
    pub methods: &'static OpaqueMethodCodegenSet,
    pub host_call_fn: &'static str,
    pub hook_bridge_fn: &'static str,
}

pub trait QtOpaqueCodegenDecl {
    const CODEGEN: OpaqueCodegenDecl;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCodegenLowering {
    pub extra_includes: &'static [&'static str],
    pub body: &'static str,
}

pub trait QtOpaqueCodegenBridge {
    const HOST_CALL_FN: &'static str;
    const HOOK_BRIDGE_FN: &'static str;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetHostOverrideCodegenMeta {
    pub rust_name: &'static str,
    pub target_name: &'static str,
    pub opaque: QtOpaqueInfo,
    pub bridge_fn: &'static str,
    pub signature: &'static str,
    pub lowering: HostCodegenLowering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetHostOverrideCodegenSet {
    pub overrides: &'static [WidgetHostOverrideCodegenMeta],
}

pub const NO_WIDGET_HOST_OVERRIDES: WidgetHostOverrideCodegenSet =
    WidgetHostOverrideCodegenSet { overrides: &[] };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetHostEventMountCodegenMeta {
    pub rust_name: &'static str,
    pub event_lower_name: &'static str,
    pub lowering: HostCodegenLowering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetHostEventMountCodegenSet {
    pub mounts: &'static [WidgetHostEventMountCodegenMeta],
}

pub const NO_WIDGET_HOST_EVENT_MOUNTS: WidgetHostEventMountCodegenSet =
    WidgetHostEventMountCodegenSet { mounts: &[] };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetHostPropSetterCodegenMeta {
    pub rust_name: &'static str,
    pub prop_lower_name: &'static str,
    pub arg_name: &'static str,
    pub value_type: crate::schema::QtTypeInfo,
    pub lowering: HostCodegenLowering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetHostPropSetterCodegenSet {
    pub setters: &'static [WidgetHostPropSetterCodegenMeta],
}

pub const NO_WIDGET_HOST_PROP_SETTERS: WidgetHostPropSetterCodegenSet =
    WidgetHostPropSetterCodegenSet { setters: &[] };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetHostPropGetterCodegenMeta {
    pub rust_name: &'static str,
    pub prop_lower_name: &'static str,
    pub value_type: crate::schema::QtTypeInfo,
    pub lowering: HostCodegenLowering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetHostPropGetterCodegenSet {
    pub getters: &'static [WidgetHostPropGetterCodegenMeta],
}

pub const NO_WIDGET_HOST_PROP_GETTERS: WidgetHostPropGetterCodegenSet =
    WidgetHostPropGetterCodegenSet { getters: &[] };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostCapabilityCodegenDecl {
    pub host_overrides: &'static WidgetHostOverrideCodegenSet,
    pub host_event_mounts: &'static WidgetHostEventMountCodegenSet,
    pub host_prop_setters: &'static WidgetHostPropSetterCodegenSet,
    pub host_prop_getters: &'static WidgetHostPropGetterCodegenSet,
}

pub trait QtHostCodegenDecl {
    fn decl() -> &'static HostCapabilityCodegenDecl;
}

pub const NO_HOST_CAPABILITY_CODEGEN_DECL: HostCapabilityCodegenDecl = HostCapabilityCodegenDecl {
    host_overrides: &NO_WIDGET_HOST_OVERRIDES,
    host_event_mounts: &NO_WIDGET_HOST_EVENT_MOUNTS,
    host_prop_setters: &NO_WIDGET_HOST_PROP_SETTERS,
    host_prop_getters: &NO_WIDGET_HOST_PROP_GETTERS,
};

#[derive(Debug, Clone, Copy)]
pub struct WidgetCodegenFragment {
    pub spec_key: SpecWidgetKey,
    pub decl: fn() -> &'static HostCapabilityCodegenDecl,
}

#[linkme::distributed_slice]
pub static QT_WIDGET_CODEGEN_FRAGMENTS: [&'static WidgetCodegenFragment];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetCodegenDecl {
    pub spec_key: SpecWidgetKey,
    pub host_overrides: &'static WidgetHostOverrideCodegenSet,
    pub host_event_mounts: &'static WidgetHostEventMountCodegenSet,
    pub host_prop_setters: &'static WidgetHostPropSetterCodegenSet,
    pub host_prop_getters: &'static WidgetHostPropGetterCodegenSet,
}

#[derive(Default)]
struct HostCodegenDeclBuilder {
    host_overrides: Vec<WidgetHostOverrideCodegenMeta>,
    host_event_mounts: Vec<WidgetHostEventMountCodegenMeta>,
    host_prop_setters: Vec<WidgetHostPropSetterCodegenMeta>,
    host_prop_getters: Vec<WidgetHostPropGetterCodegenMeta>,
}

impl HostCodegenDeclBuilder {
    fn extend(&mut self, context: &str, decl: &'static HostCapabilityCodegenDecl) {
        for meta in decl.host_overrides.overrides {
            if self
                .host_overrides
                .iter()
                .any(|existing| existing.target_name == meta.target_name)
            {
                panic!(
                    "duplicate host override {} for {}",
                    meta.target_name, context
                );
            }
            self.host_overrides.push(*meta);
        }

        for meta in decl.host_event_mounts.mounts {
            if self
                .host_event_mounts
                .iter()
                .any(|existing| existing.event_lower_name == meta.event_lower_name)
            {
                panic!(
                    "duplicate host event mount {} for {}",
                    meta.event_lower_name, context
                );
            }
            self.host_event_mounts.push(*meta);
        }

        for meta in decl.host_prop_setters.setters {
            if self
                .host_prop_setters
                .iter()
                .any(|existing| existing.prop_lower_name == meta.prop_lower_name)
            {
                panic!(
                    "duplicate host prop setter {} for {}",
                    meta.prop_lower_name, context
                );
            }
            self.host_prop_setters.push(*meta);
        }

        for meta in decl.host_prop_getters.getters {
            if self
                .host_prop_getters
                .iter()
                .any(|existing| existing.prop_lower_name == meta.prop_lower_name)
            {
                panic!(
                    "duplicate host prop getter {} for {}",
                    meta.prop_lower_name, context
                );
            }
            self.host_prop_getters.push(*meta);
        }
    }

    fn finish(self) -> &'static HostCapabilityCodegenDecl {
        Box::leak(Box::new(HostCapabilityCodegenDecl {
            host_overrides: Box::leak(Box::new(WidgetHostOverrideCodegenSet {
                overrides: Box::leak(self.host_overrides.into_boxed_slice()),
            })),
            host_event_mounts: Box::leak(Box::new(WidgetHostEventMountCodegenSet {
                mounts: Box::leak(self.host_event_mounts.into_boxed_slice()),
            })),
            host_prop_setters: Box::leak(Box::new(WidgetHostPropSetterCodegenSet {
                setters: Box::leak(self.host_prop_setters.into_boxed_slice()),
            })),
            host_prop_getters: Box::leak(Box::new(WidgetHostPropGetterCodegenSet {
                getters: Box::leak(self.host_prop_getters.into_boxed_slice()),
            })),
        }))
    }
}

pub fn merge_host_codegen_decls(
    context: &str,
    decls: &[&'static HostCapabilityCodegenDecl],
) -> &'static HostCapabilityCodegenDecl {
    let mut builder = HostCodegenDeclBuilder::default();
    for decl in decls {
        builder.extend(context, decl);
    }
    builder.finish()
}
