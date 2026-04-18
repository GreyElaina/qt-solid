use std::{any::TypeId, collections::BTreeMap, sync::OnceLock};

use crate::decl::{
    AlignItems, FlexDirection, FocusPolicy, JustifyContent, NodeClass, SpecWidgetKey, WidgetTypeId,
};
use crate::runtime::{FontWeight, NonNegativeF64, QtOpaqueInfo};

pub use linkme;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChildrenKind {
    None,
    Text,
    Nodes,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EnumMeta {
    pub name: &'static str,
    pub values: &'static [&'static str],
}

pub trait QtEnumDomain {
    const META: &'static EnumMeta;
}

pub fn enum_tag_for_value(meta: &EnumMeta, value: &str) -> Option<i32> {
    meta.values
        .iter()
        .position(|candidate| *candidate == value)
        .and_then(|index| i32::try_from(index + 1).ok())
}

pub fn enum_value_for_tag(meta: &EnumMeta, tag: i32) -> Option<&'static str> {
    if tag <= 0 {
        return None;
    }
    let index = usize::try_from(tag - 1).ok()?;
    meta.values.get(index).copied()
}

const FLEX_DIRECTION_ENUM: EnumMeta = EnumMeta {
    name: "FlexDirection",
    values: &["column", "row"],
};

const ALIGN_ITEMS_ENUM: EnumMeta = EnumMeta {
    name: "AlignItems",
    values: &["flex-start", "center", "flex-end", "stretch"],
};

const JUSTIFY_CONTENT_ENUM: EnumMeta = EnumMeta {
    name: "JustifyContent",
    values: &["flex-start", "center", "flex-end"],
};

const FOCUS_POLICY_ENUM: EnumMeta = EnumMeta {
    name: "FocusPolicy",
    values: &["no-focus", "tab-focus", "click-focus", "strong-focus"],
};

impl QtEnumDomain for FlexDirection {
    const META: &'static EnumMeta = &FLEX_DIRECTION_ENUM;
}

impl QtEnumDomain for AlignItems {
    const META: &'static EnumMeta = &ALIGN_ITEMS_ENUM;
}

impl QtEnumDomain for JustifyContent {
    const META: &'static EnumMeta = &JUSTIFY_CONTENT_ENUM;
}

impl QtEnumDomain for FocusPolicy {
    const META: &'static EnumMeta = &FOCUS_POLICY_ENUM;
}

fn type_id_string() -> TypeId {
    TypeId::of::<String>()
}

fn type_id_bool() -> TypeId {
    TypeId::of::<bool>()
}

fn type_id_i32() -> TypeId {
    TypeId::of::<i32>()
}

fn type_id_u32() -> TypeId {
    TypeId::of::<u32>()
}

fn type_id_f64() -> TypeId {
    TypeId::of::<f64>()
}

fn type_id_non_negative_f64() -> TypeId {
    TypeId::of::<NonNegativeF64>()
}

fn type_id_unit() -> TypeId {
    TypeId::of::<()>()
}

fn type_id_of<T: 'static>() -> TypeId {
    TypeId::of::<T>()
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtValueRepr {
    Unit,
    String,
    Bool,
    I32 { non_negative: bool },
    F64 { non_negative: bool },
    Enum(&'static EnumMeta),
    Color,
    Point,
    Size,
    Rect,
    Affine,
}

#[derive(Debug, Clone, Copy)]
pub struct QtTypeInfo {
    rust_path: &'static str,
    repr: QtValueRepr,
    type_id: fn() -> TypeId,
}

impl PartialEq for QtTypeInfo {
    fn eq(&self, other: &Self) -> bool {
        self.rust_path == other.rust_path
            && self.repr == other.repr
            && self.type_id() == other.type_id()
    }
}

impl Eq for QtTypeInfo {}

impl QtTypeInfo {
    pub const fn new(rust_path: &'static str, repr: QtValueRepr, type_id: fn() -> TypeId) -> Self {
        Self {
            rust_path,
            repr,
            type_id,
        }
    }

    pub const fn rust_path(self) -> &'static str {
        self.rust_path
    }

    pub const fn repr(self) -> QtValueRepr {
        self.repr
    }

    pub const fn ts_type(self) -> &'static str {
        match self.repr {
            QtValueRepr::Unit => "void",
            QtValueRepr::String => "string",
            QtValueRepr::Bool => "boolean",
            QtValueRepr::I32 { .. } | QtValueRepr::F64 { .. } => "number",
            QtValueRepr::Enum(_) => "enum",
            QtValueRepr::Color => "QtColor",
            QtValueRepr::Point => "QtPoint",
            QtValueRepr::Size => "QtSize",
            QtValueRepr::Rect => "QtRect",
            QtValueRepr::Affine => "QtAffine",
        }
    }

    pub const fn enum_meta(self) -> Option<&'static EnumMeta> {
        match self.repr {
            QtValueRepr::Enum(meta) => Some(meta),
            _ => None,
        }
    }

    pub fn accepts_qt_value(self, value: &crate::runtime::QtValue) -> bool {
        match (self.repr, value) {
            (QtValueRepr::Unit, crate::runtime::QtValue::Unit) => true,
            (QtValueRepr::String, crate::runtime::QtValue::String(_)) => true,
            (QtValueRepr::Bool, crate::runtime::QtValue::Bool(_)) => true,
            (QtValueRepr::I32 { .. }, crate::runtime::QtValue::I32(_)) => true,
            (QtValueRepr::F64 { .. }, crate::runtime::QtValue::F64(_)) => true,
            (QtValueRepr::Enum(_), crate::runtime::QtValue::Enum(_)) => true,
            (QtValueRepr::Color, crate::runtime::QtValue::Color(_)) => true,
            (QtValueRepr::Point, crate::runtime::QtValue::Point(_)) => true,
            (QtValueRepr::Size, crate::runtime::QtValue::Size(_)) => true,
            (QtValueRepr::Rect, crate::runtime::QtValue::Rect(_)) => true,
            (QtValueRepr::Affine, crate::runtime::QtValue::Affine(_)) => true,
            _ => false,
        }
    }

    pub const fn is_non_negative(self) -> bool {
        match self.repr {
            QtValueRepr::I32 { non_negative } | QtValueRepr::F64 { non_negative } => non_negative,
            _ => false,
        }
    }

    pub fn type_id(self) -> TypeId {
        (self.type_id)()
    }
}

pub trait QtType {
    const INFO: QtTypeInfo;
}

impl QtType for String {
    const INFO: QtTypeInfo = QtTypeInfo::new("String", QtValueRepr::String, type_id_string);
}

impl QtType for &str {
    const INFO: QtTypeInfo = QtTypeInfo::new("&str", QtValueRepr::String, type_id_string);
}

impl QtType for bool {
    const INFO: QtTypeInfo = QtTypeInfo::new("bool", QtValueRepr::Bool, type_id_bool);
}

impl QtType for i32 {
    const INFO: QtTypeInfo = QtTypeInfo::new(
        "i32",
        QtValueRepr::I32 {
            non_negative: false,
        },
        type_id_i32,
    );
}

impl QtType for u32 {
    const INFO: QtTypeInfo =
        QtTypeInfo::new("u32", QtValueRepr::I32 { non_negative: true }, type_id_u32);
}

impl QtType for f64 {
    const INFO: QtTypeInfo = QtTypeInfo::new(
        "f64",
        QtValueRepr::F64 {
            non_negative: false,
        },
        type_id_f64,
    );
}

impl QtType for NonNegativeF64 {
    const INFO: QtTypeInfo = QtTypeInfo::new(
        "qt_solid_widget_core::runtime::NonNegativeF64",
        QtValueRepr::F64 { non_negative: true },
        type_id_non_negative_f64,
    );
}

impl QtType for FontWeight {
    const INFO: QtTypeInfo = QtTypeInfo::new(
        "qt_solid_widget_core::runtime::FontWeight",
        QtValueRepr::I32 { non_negative: true },
        type_id_of::<FontWeight>,
    );
}

impl QtType for () {
    const INFO: QtTypeInfo = QtTypeInfo::new("()", QtValueRepr::Unit, type_id_unit);
}

impl<T> QtType for T
where
    T: QtEnumDomain + 'static,
{
    const INFO: QtTypeInfo =
        QtTypeInfo::new(T::META.name, QtValueRepr::Enum(T::META), type_id_of::<T>);
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropBehavior {
    State,
    Const,
    Command,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropLowerKind {
    MetaProperty = 1,
    Custom = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PropLowering {
    MetaProperty(&'static str),
    Custom(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PropMeta {
    pub index: u8,
    pub rust_name: &'static str,
    pub path: &'static str,
    pub js_name: &'static str,
    pub symbol: &'static str,
    pub value_type: QtTypeInfo,
    pub optional: bool,
    pub lowering: PropLowering,
    pub read_lowering: Option<PropLowering>,
    pub behavior: PropBehavior,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventPayloadKind {
    Unit = 0,
    Scalar = 1,
    Object = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventFieldMeta {
    pub rust_name: &'static str,
    pub js_name: &'static str,
    pub value_type: QtTypeInfo,
}

#[repr(u8)]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventLowerKind {
    QtSignal = 1,
    Custom = 2,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EventLowering {
    QtSignal(&'static str),
    Custom(&'static str),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMeta {
    pub index: u8,
    pub rust_name: &'static str,
    pub exports: &'static [&'static str],
    pub label: &'static str,
    pub payload_kind: EventPayloadKind,
    pub payload_type: Option<QtTypeInfo>,
    pub payload_fields: Vec<EventFieldMeta>,
    pub lowering: EventLowering,
    pub echoes: Vec<EventEchoMeta>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventEchoMeta {
    pub prop_js_name: &'static str,
    pub value_path: &'static str,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostMeta {
    pub class: &'static str,
    pub include: &'static str,
    pub factory: Option<&'static str>,
    pub top_level: bool,
}

impl HostMeta {
    pub const fn root() -> Self {
        Self {
            class: "",
            include: "",
            factory: None,
            top_level: false,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetLayoutKind {
    Box,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecLeafProp {
    pub rust_name: &'static str,
    pub js_name: &'static str,
    pub value_type: QtTypeInfo,
    pub optional: bool,
    pub lowering: PropLowering,
    pub read_lowering: Option<PropLowering>,
    pub behavior: PropBehavior,
    pub exported: bool,
    pub default: SpecPropDefaultValue,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpecPropNode {
    Leaf(SpecLeafProp),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecPropTree {
    pub type_name: &'static str,
    pub nodes: &'static [SpecPropNode],
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum SpecPropDefaultValue {
    None,
    Bool(bool),
    I32(i32),
    F64(f64),
    String(&'static str),
    Enum(&'static str),
}

pub trait QtPropTree: Clone {
    fn spec() -> &'static SpecPropTree;
}

#[derive(Debug, Clone, Copy, Default)]
pub struct NoProps;

impl QtPropTree for NoProps {
    fn spec() -> &'static SpecPropTree {
        static SPEC: SpecPropTree = SpecPropTree {
            type_name: "NoProps",
            nodes: &[],
        };

        &SPEC
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecCreateProp {
    pub key: &'static str,
    pub value_type: QtTypeInfo,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecPropDecl {
    pub path: &'static [&'static str],
    pub value_type: QtTypeInfo,
    pub default: SpecPropDefaultValue,
    pub init_setter_slot: Option<u16>,
    pub setter_slot: Option<u16>,
    pub getter_slot: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecEventMeta {
    pub index: u8,
    pub rust_name: &'static str,
    pub exports: &'static [&'static str],
    pub payload_kind: EventPayloadKind,
    pub payload_type: Option<QtTypeInfo>,
    pub payload_fields: &'static [EventFieldMeta],
    pub echoes: &'static [EventEchoMeta],
    pub label: &'static str,
    pub lowering: EventLowering,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecEventSet {
    pub uses: &'static [&'static str],
    pub events: &'static [SpecEventMeta],
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecHostMethodArg {
    pub rust_name: &'static str,
    pub js_name: &'static str,
    pub value_type: QtTypeInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecHostMethodMeta {
    pub slot: u16,
    pub rust_name: &'static str,
    pub js_name: &'static str,
    pub host_name: &'static str,
    pub args: &'static [SpecHostMethodArg],
    pub return_type: QtTypeInfo,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecHostMethodSet {
    pub methods: &'static [SpecHostMethodMeta],
}

pub const NO_HOST_METHODS: SpecHostMethodSet = SpecHostMethodSet { methods: &[] };

pub trait QtHostMethodSurface {
    const SPEC: SpecHostMethodSet;
}

pub struct NoHostMethods;

impl QtHostMethodSurface for NoHostMethods {
    const SPEC: SpecHostMethodSet = NO_HOST_METHODS;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecMethodSet {
    pub host_methods: &'static [SpecHostMethodMeta],
}

pub const NO_METHODS: SpecMethodSet = SpecMethodSet { host_methods: &[] };

pub trait QtMethodSet {
    const SPEC: SpecMethodSet;
}

pub struct NoMethods;

impl QtMethodSet for NoMethods {
    const SPEC: SpecMethodSet = NO_METHODS;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpecOpaqueDecl {
    pub opaque: QtOpaqueInfo,
    pub methods: &'static SpecHostMethodSet,
}

pub trait QtOpaqueDecl {
    const SPEC: SpecOpaqueDecl;
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecWidgetCore {
    pub spec_key: SpecWidgetKey,
    pub kind_name: &'static str,
    pub type_name: &'static str,
    pub children: ChildrenKind,
    pub props: &'static SpecPropTree,
}

pub fn fallback_widget_type_id(name: &str) -> WidgetTypeId {
    let bytes = name.as_bytes();
    let mut hash = 0x811c9dc5u32;
    let mut index = 0usize;

    while index < bytes.len() {
        hash ^= bytes[index] as u32;
        hash = hash.wrapping_mul(0x0100_0193);
        index += 1;
    }

    if hash == 0 {
        WidgetTypeId::new(u32::MAX)
    } else {
        WidgetTypeId::new(hash)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct SpecWidgetBinding {
    pub spec_key: SpecWidgetKey,
    pub kind_name: &'static str,
    pub type_name: &'static str,
    pub children: ChildrenKind,
    pub host: HostMeta,
    pub default_layout: Option<WidgetLayoutKind>,
    pub props: &'static SpecPropTree,
    pub events: &'static SpecEventSet,
    pub methods: &'static SpecMethodSet,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct HostCapabilitySpecDecl {
    pub host: Option<HostMeta>,
    pub default_layout: Option<WidgetLayoutKind>,
    pub props: &'static [SpecLeafProp],
    pub events: &'static [SpecEventMeta],
    pub methods: &'static SpecMethodSet,
}

pub trait QtHostSpecDecl {
    fn decl() -> &'static HostCapabilitySpecDecl;
}

pub const NO_HOST_CAPABILITY_SPEC_DECL: HostCapabilitySpecDecl = HostCapabilitySpecDecl {
    host: None,
    default_layout: None,
    props: &[],
    events: &[],
    methods: &NO_METHODS,
};

#[derive(Debug, Clone, Copy)]
pub struct WidgetHostSpecFragment {
    pub spec_key: SpecWidgetKey,
    pub decl: fn() -> &'static HostCapabilitySpecDecl,
}

#[linkme::distributed_slice]
pub static QT_WIDGET_HOST_SPEC_FRAGMENTS: [&'static WidgetHostSpecFragment];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetBinding {
    pub class: NodeClass,
    pub spec_key: SpecWidgetKey,
    pub widget_type_id: WidgetTypeId,
    pub kind_name: &'static str,
    pub type_name: &'static str,
    pub children: ChildrenKind,
    pub host: HostMeta,
    pub default_layout: Option<WidgetLayoutKind>,
    pub props: Vec<PropMeta>,
    pub events: Vec<EventMeta>,
    pub methods: &'static SpecMethodSet,
}

#[derive(Debug, Clone, Copy)]
pub struct WidgetLibraryBindings {
    pub library_key: &'static str,
    pub spec_bindings: &'static [&'static SpecWidgetBinding],
    pub opaque_decls: &'static [&'static SpecOpaqueDecl],
    pub opaque_codegen_decls: &'static [&'static crate::codegen::OpaqueCodegenDecl],
    pub widget_native_decls: &'static [&'static crate::runtime::WidgetNativeDecl],
    pub widget_prop_decls: &'static [&'static crate::runtime::WidgetPropDecl],
}

pub trait QtWidgetDecl {
    type Props: QtPropTree;

    fn spec() -> &'static SpecWidgetBinding;

    fn binding() -> &'static WidgetBinding;

    fn widget_type_id() -> WidgetTypeId {
        Self::binding().widget_type_id
    }
}

#[derive(Default)]
struct WidgetHostSpecBuilder {
    host: Option<HostMeta>,
    default_layout: Option<WidgetLayoutKind>,
    props: Vec<SpecLeafProp>,
    events: Vec<SpecEventMeta>,
    host_methods: Vec<SpecHostMethodMeta>,
}

impl WidgetHostSpecBuilder {
    fn extend(&mut self, context: &str, decl: &'static HostCapabilitySpecDecl) {
        if let Some(host) = decl.host {
            if self.host.replace(host).is_some() {
                panic!("duplicate host declaration for {context}");
            }
        }

        if let Some(default_layout) = decl.default_layout {
            if let Some(existing) = self.default_layout {
                if existing != default_layout {
                    panic!("conflicting default layout for {context}");
                }
            } else {
                self.default_layout = Some(default_layout);
            }
        }

        for prop in decl.props {
            if self
                .props
                .iter()
                .any(|existing| existing.js_name == prop.js_name)
            {
                panic!("duplicate widget prop {} for {}", prop.js_name, context);
            }
            self.props.push(*prop);
        }

        for event in decl.events {
            if self
                .events
                .iter()
                .any(|existing| existing.label == event.label)
            {
                panic!("duplicate host event {} for {}", event.label, context);
            }

            for export in event.exports {
                if self
                    .events
                    .iter()
                    .any(|existing| existing.exports.contains(export))
                {
                    panic!("duplicate host event export {} for {}", export, context);
                }
            }

            self.events.push(*event);
        }

        for method in decl.methods.host_methods {
            if self
                .host_methods
                .iter()
                .any(|existing| existing.js_name == method.js_name)
            {
                panic!(
                    "duplicate widget method export {} for {}",
                    method.js_name, context
                );
            }
            if self
                .host_methods
                .iter()
                .any(|existing| existing.host_name == method.host_name)
            {
                panic!(
                    "duplicate widget host method {} for {}",
                    method.host_name, context
                );
            }
            self.host_methods.push(*method);
        }
    }

    fn finish_decl(self) -> &'static HostCapabilitySpecDecl {
        Box::leak(Box::new(HostCapabilitySpecDecl {
            host: self.host,
            default_layout: self.default_layout,
            props: leak_reindexed_host_props(self.props),
            events: leak_reindexed_events(self.events),
            methods: Box::leak(Box::new(SpecMethodSet {
                host_methods: leak_reindexed_host_methods(self.host_methods),
            })),
        }))
    }

    fn finish_widget(
        self,
        spec_key: SpecWidgetKey,
    ) -> (
        HostMeta,
        Option<WidgetLayoutKind>,
        &'static [SpecLeafProp],
        &'static [SpecEventMeta],
        &'static SpecMethodSet,
    ) {
        let host = self.host.unwrap_or_else(|| {
            panic!(
                "missing host declaration for spec widget key {}",
                spec_key.raw()
            )
        });
        let props = leak_reindexed_host_props(self.props);
        let events = leak_reindexed_events(self.events);
        let methods = Box::leak(Box::new(SpecMethodSet {
            host_methods: leak_reindexed_host_methods(self.host_methods),
        }));
        (host, self.default_layout, props, events, methods)
    }
}

fn leak_reindexed_host_props(mut props: Vec<SpecLeafProp>) -> &'static [SpecLeafProp] {
    props.sort_by(|left, right| {
        (left.js_name, left.rust_name).cmp(&(right.js_name, right.rust_name))
    });

    Box::leak(props.into_boxed_slice())
}

fn leak_reindexed_events(mut events: Vec<SpecEventMeta>) -> &'static [SpecEventMeta] {
    events.sort_by(|left, right| (left.label, left.rust_name).cmp(&(right.label, right.rust_name)));

    let events = events
        .into_iter()
        .enumerate()
        .map(|(index, event)| SpecEventMeta {
            index: u8::try_from(index).expect("host event count exceeds current schema limit"),
            ..event
        })
        .collect::<Vec<_>>();

    Box::leak(events.into_boxed_slice())
}

fn leak_reindexed_host_methods(
    mut methods: Vec<SpecHostMethodMeta>,
) -> &'static [SpecHostMethodMeta] {
    methods.sort_by(|left, right| {
        (left.js_name, left.host_name, left.rust_name).cmp(&(
            right.js_name,
            right.host_name,
            right.rust_name,
        ))
    });

    let methods = methods
        .into_iter()
        .enumerate()
        .map(|(index, method)| SpecHostMethodMeta {
            slot: u16::try_from(index).expect("host method count exceeds current schema limit"),
            ..method
        })
        .collect::<Vec<_>>();

    Box::leak(methods.into_boxed_slice())
}

pub fn merge_host_spec_decls(
    context: &str,
    decls: &[&'static HostCapabilitySpecDecl],
) -> &'static HostCapabilitySpecDecl {
    let mut builder = WidgetHostSpecBuilder::default();
    for decl in decls {
        builder.extend(context, decl);
    }
    builder.finish_decl()
}

fn resolve_widget_host_spec(
    spec_key: SpecWidgetKey,
) -> (
    HostMeta,
    Option<WidgetLayoutKind>,
    &'static [SpecLeafProp],
    &'static [SpecEventMeta],
    &'static SpecMethodSet,
) {
    let mut builder = WidgetHostSpecBuilder::default();

    for fragment in QT_WIDGET_HOST_SPEC_FRAGMENTS.iter().copied() {
        if fragment.spec_key != spec_key {
            continue;
        }
        builder.extend(spec_key.raw(), (fragment.decl)());
    }

    builder.finish_widget(spec_key)
}

fn merge_leaf_prop(context: &str, props: &mut Vec<SpecLeafProp>, prop: SpecLeafProp) {
    if let Some(existing) = props
        .iter_mut()
        .find(|existing| existing.js_name == prop.js_name)
    {
        if existing.value_type != prop.value_type {
            panic!(
                "widget prop {} mixes incompatible value types for {}",
                prop.js_name, context
            );
        }
        if existing.lowering != prop.lowering {
            panic!(
                "widget prop {} mixes incompatible lowering for {}",
                prop.js_name, context
            );
        }
        match (existing.read_lowering, prop.read_lowering) {
            (Some(left), Some(right)) if left != right => {
                panic!(
                    "widget prop {} mixes incompatible read lowering for {}",
                    prop.js_name, context
                );
            }
            (None, Some(read_lowering)) => {
                existing.read_lowering = Some(read_lowering);
            }
            _ => {}
        }
        if existing.behavior != prop.behavior {
            panic!(
                "widget prop {} mixes incompatible behaviors for {}",
                prop.js_name, context
            );
        }
        match (existing.default, prop.default) {
            (SpecPropDefaultValue::None, default) => {
                existing.default = default;
            }
            (left, right) if right != SpecPropDefaultValue::None && left != right => {
                panic!(
                    "widget prop {} declares duplicate defaults for {}",
                    prop.js_name, context
                );
            }
            _ => {}
        }
        existing.optional |= prop.optional;
        existing.exported |= prop.exported;
        return;
    }

    props.push(prop);
}

pub fn resolve_widget_prop_spec(
    spec_key: SpecWidgetKey,
    type_name: &'static str,
    core: &'static SpecPropTree,
) -> &'static SpecPropTree {
    let mut props = Vec::<SpecLeafProp>::new();
    let (_, _, host_props, _, _) = resolve_widget_host_spec(spec_key);

    for node in core.nodes {
        let SpecPropNode::Leaf(prop) = node;
        merge_leaf_prop(type_name, &mut props, *prop);
    }

    for prop in host_props {
        merge_leaf_prop(type_name, &mut props, *prop);
    }

    if host_props.is_empty() {
        return core;
    }

    let nodes = leak_reindexed_host_props(props)
        .iter()
        .copied()
        .map(SpecPropNode::Leaf)
        .collect::<Vec<_>>();

    Box::leak(Box::new(SpecPropTree {
        type_name,
        nodes: Box::leak(nodes.into_boxed_slice()),
    }))
}

#[cfg(test)]
fn merged_widget_props(
    type_name: &'static str,
    core: &'static SpecPropTree,
    host_props: &'static [SpecLeafProp],
) -> &'static SpecPropTree {
    let mut props = Vec::<SpecLeafProp>::new();

    for node in core.nodes {
        let SpecPropNode::Leaf(prop) = node;
        merge_leaf_prop(type_name, &mut props, *prop);
    }

    for prop in host_props {
        merge_leaf_prop(type_name, &mut props, *prop);
    }

    let nodes = leak_reindexed_host_props(props)
        .iter()
        .copied()
        .map(SpecPropNode::Leaf)
        .collect::<Vec<_>>();

    Box::leak(Box::new(SpecPropTree {
        type_name,
        nodes: Box::leak(nodes.into_boxed_slice()),
    }))
}

pub fn resolve_widget_spec(core: &SpecWidgetCore) -> SpecWidgetBinding {
    let (host, default_layout, _host_props, host_events, host_methods) =
        resolve_widget_host_spec(core.spec_key);
    let props = resolve_widget_prop_spec(core.spec_key, core.type_name, core.props);
    let events = Box::leak(Box::new(SpecEventSet {
        uses: &[],
        events: leak_reindexed_events(host_events.to_vec()),
    }));
    SpecWidgetBinding {
        spec_key: core.spec_key,
        kind_name: core.kind_name,
        type_name: core.type_name,
        children: core.children,
        host,
        default_layout,
        props,
        events,
        methods: host_methods,
    }
}

fn path_to_js_name(path: &[&'static str]) -> String {
    let mut result = String::new();

    for (index, segment) in path.iter().enumerate() {
        if index == 0 {
            result.push_str(segment);
            continue;
        }

        let mut chars = segment.chars();
        if let Some(first) = chars.next() {
            result.extend(first.to_uppercase());
        }
        result.extend(chars);
    }

    result
}

#[derive(Debug, Clone, PartialEq)]
pub struct ExportedProp {
    pub path: Vec<&'static str>,
    pub key: String,
    pub value_type: QtTypeInfo,
    pub default: SpecPropDefaultValue,
}

#[derive(Debug, Clone, PartialEq)]
pub struct MergedProp {
    pub path: Vec<&'static str>,
    pub key: String,
    pub value_type: QtTypeInfo,
    pub is_bound: bool,
    pub default: SpecPropDefaultValue,
    pub init_setter_slot: Option<u16>,
    pub setter_slot: Option<u16>,
    pub getter_slot: Option<u16>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EndpointWriteMode {
    None,
    CreateOnly(u16),
    Live(u16),
    Bound,
}

impl MergedProp {
    fn from_exported(exported: ExportedProp) -> Self {
        Self {
            path: exported.path,
            key: exported.key,
            value_type: exported.value_type,
            is_bound: true,
            default: exported.default,
            init_setter_slot: None,
            setter_slot: None,
            getter_slot: None,
        }
    }

    fn from_decl(prop: &SpecPropDecl) -> Self {
        let path = prop.path.to_vec();
        Self {
            key: path_to_js_name(&path),
            path,
            value_type: prop.value_type,
            is_bound: false,
            default: prop.default,
            init_setter_slot: prop.init_setter_slot,
            setter_slot: prop.setter_slot,
            getter_slot: prop.getter_slot,
        }
    }

    fn overlay_decl(&mut self, prop: &SpecPropDecl) {
        let path = prop.path.to_vec();
        if self.path != path {
            panic!(
                "prop key {} resolved to mismatched paths {:?} and {:?}",
                self.key, self.path, path
            );
        }
        if self.value_type != prop.value_type {
            panic!(
                "prop {} type mismatch: {} vs {}",
                self.key,
                self.value_type.rust_path(),
                prop.value_type.rust_path()
            );
        }
        if !matches!(prop.default, SpecPropDefaultValue::None) {
            self.default = prop.default;
        }
        if prop.init_setter_slot.is_some() {
            self.init_setter_slot = prop.init_setter_slot;
        }
        if prop.setter_slot.is_some() {
            self.setter_slot = prop.setter_slot;
        }
        if prop.getter_slot.is_some() {
            self.getter_slot = prop.getter_slot;
        }
    }

    pub fn write_mode(&self) -> EndpointWriteMode {
        if self.is_bound {
            return EndpointWriteMode::Bound;
        }
        if let Some(slot) = self.setter_slot {
            return EndpointWriteMode::Live(slot);
        }
        if let Some(slot) = self.init_setter_slot {
            return EndpointWriteMode::CreateOnly(slot);
        }
        EndpointWriteMode::None
    }

    pub fn write_slot(&self) -> Option<u16> {
        match self.write_mode() {
            EndpointWriteMode::CreateOnly(slot) | EndpointWriteMode::Live(slot) => Some(slot),
            EndpointWriteMode::None | EndpointWriteMode::Bound => None,
        }
    }

    pub fn init_only_slot(&self) -> Option<u16> {
        match self.write_mode() {
            EndpointWriteMode::CreateOnly(slot) => Some(slot),
            EndpointWriteMode::None | EndpointWriteMode::Live(_) | EndpointWriteMode::Bound => None,
        }
    }

    pub fn read_slot(&self) -> Option<u16> {
        self.getter_slot
    }

    pub fn has_live_write(&self) -> bool {
        matches!(
            self.write_mode(),
            EndpointWriteMode::Bound | EndpointWriteMode::Live(_)
        )
    }
}

fn push_exported_props(nodes: &[SpecPropNode], props: &mut Vec<ExportedProp>) {
    for node in nodes {
        let SpecPropNode::Leaf(leaf) = node;
        if !leaf.exported {
            continue;
        }
        props.push(ExportedProp {
            path: vec![leaf.js_name],
            key: leaf.js_name.to_owned(),
            value_type: leaf.value_type,
            default: leaf.default,
        });
    }
}

pub fn exported_props(spec: &SpecPropTree) -> Vec<ExportedProp> {
    let mut props = Vec::new();
    push_exported_props(spec.nodes, &mut props);
    props
}

pub fn merged_prop_decls(
    spec_key: SpecWidgetKey,
    decl: Option<&crate::runtime::WidgetPropDecl>,
) -> Vec<MergedProp> {
    let mut props = BTreeMap::<String, MergedProp>::new();

    if let Some(decl) = decl {
        for prop in decl.props {
            let key = path_to_js_name(prop.path);
            match props.entry(key.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(MergedProp::from_decl(prop));
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().overlay_decl(prop);
                }
            }
        }
    }

    for fragment in crate::runtime::QT_WIDGET_PROP_DECL_FRAGMENTS
        .iter()
        .copied()
    {
        if fragment.spec_key != spec_key {
            continue;
        }

        for prop in (fragment.decl)() {
            let key = path_to_js_name(prop.path);
            match props.entry(key.clone()) {
                std::collections::btree_map::Entry::Vacant(entry) => {
                    entry.insert(MergedProp::from_decl(prop));
                }
                std::collections::btree_map::Entry::Occupied(mut entry) => {
                    entry.get_mut().overlay_decl(prop);
                }
            }
        }
    }

    props.into_values().collect()
}

pub fn merged_props(
    spec: &SpecWidgetBinding,
    decl: Option<&crate::runtime::WidgetPropDecl>,
) -> Vec<MergedProp> {
    let mut props = BTreeMap::<String, MergedProp>::new();

    for exported in exported_props(spec.props) {
        props.insert(exported.key.clone(), MergedProp::from_exported(exported));
    }

    for prop in merged_prop_decls(spec.spec_key, decl) {
        match props.entry(prop.key.clone()) {
            std::collections::btree_map::Entry::Vacant(entry) => {
                entry.insert(prop);
            }
            std::collections::btree_map::Entry::Occupied(mut entry) => {
                let existing = entry.get_mut();
                if existing.path != prop.path {
                    panic!(
                        "prop key {} resolved to mismatched paths {:?} and {:?}",
                        prop.key, existing.path, prop.path
                    );
                }
                if existing.value_type != prop.value_type {
                    panic!(
                        "prop {} type mismatch: {} vs {}",
                        prop.key, existing.value_type.rust_path, prop.value_type.rust_path
                    );
                }

                if prop.init_setter_slot.is_some() {
                    existing.init_setter_slot = prop.init_setter_slot;
                }
                if prop.setter_slot.is_some() {
                    existing.setter_slot = prop.setter_slot;
                }
                if prop.getter_slot.is_some() {
                    existing.getter_slot = prop.getter_slot;
                }
                if prop.default != SpecPropDefaultValue::None {
                    existing.default = prop.default;
                }
            }
        }
    }

    props.into_values().collect()
}

fn push_resolved_props(nodes: &[SpecPropNode], props: &mut Vec<PropMeta>) {
    for node in nodes {
        let SpecPropNode::Leaf(leaf) = node;
        let index = u8::try_from(props.len()).expect("Qt widget supports at most 255 props");
        let prop_path = leaf.js_name;
        let js_name = leaf.js_name;
        let symbol = Box::leak(
            [leaf.rust_name]
                .into_iter()
                .collect::<Vec<_>>()
                .join(".")
                .into_boxed_str(),
        );

        props.push(PropMeta {
            index,
            rust_name: leaf.rust_name,
            path: prop_path,
            js_name,
            symbol,
            value_type: leaf.value_type,
            optional: leaf.optional,
            lowering: leaf.lowering,
            read_lowering: leaf.read_lowering,
            behavior: leaf.behavior,
        });
    }
}

fn event_echo_value_type(event: &EventMeta, echo: &EventEchoMeta) -> Option<QtTypeInfo> {
    match event.payload_kind {
        EventPayloadKind::Unit => None,
        EventPayloadKind::Scalar => {
            if echo.value_path.is_empty() {
                event.payload_type
            } else {
                None
            }
        }
        EventPayloadKind::Object => event
            .payload_fields
            .iter()
            .find(|field| field.js_name == echo.value_path)
            .map(|field| field.value_type),
    }
}

fn validate_event_echoes(props: &[PropMeta], events: &[EventMeta]) {
    for event in events {
        for echo in &event.echoes {
            let prop = props
                .iter()
                .find(|prop| prop.js_name == echo.prop_js_name)
                .unwrap_or_else(|| {
                    panic!(
                        "widget event {} echoes missing prop {}",
                        event.rust_name, echo.prop_js_name
                    )
                });
            let value_type = event_echo_value_type(event, echo).unwrap_or_else(|| {
                panic!(
                    "widget event {} echoes invalid payload path {}",
                    event.rust_name, echo.value_path
                )
            });
            if prop.value_type != value_type {
                panic!(
                    "widget event {} echo {} type mismatch: {} vs {}",
                    event.rust_name,
                    echo.prop_js_name,
                    prop.value_type.rust_path,
                    value_type.rust_path
                );
            }
        }
    }
}

pub fn resolve_widget_binding(
    spec: &SpecWidgetBinding,
    widget_type_id: WidgetTypeId,
) -> WidgetBinding {
    let mut props = Vec::new();
    push_resolved_props(spec.props.nodes, &mut props);

    let mut events = Vec::new();
    for event in spec.events.events {
        events.push(EventMeta {
            index: event.index,
            rust_name: event.rust_name,
            exports: event.exports,
            label: event.label,
            payload_kind: event.payload_kind,
            payload_type: event.payload_type,
            payload_fields: event.payload_fields.to_vec(),
            lowering: event.lowering,
            echoes: event.echoes.to_vec(),
        });
    }

    validate_event_echoes(&props, &events);

    WidgetBinding {
        class: NodeClass::Widget(widget_type_id),
        spec_key: spec.spec_key,
        widget_type_id,
        kind_name: spec.kind_name,
        type_name: spec.type_name,
        children: spec.children,
        host: spec.host,
        default_layout: spec.default_layout,
        props,
        events,
        methods: spec.methods,
    }
}

#[cfg(test)]
mod tests {
    use super::{
        EndpointWriteMode, EventEchoMeta, EventFieldMeta, EventLowering, EventMeta,
        EventPayloadKind, HostMeta, MergedProp, QtEnumDomain, QtMethodSet, QtType, QtValueRepr,
        SpecPropDecl, SpecPropDefaultValue, SpecPropTree, SpecWidgetBinding, exported_props,
        merged_props, validate_event_echoes,
    };
    use crate::{decl::FlexDirection, runtime::QtValue};

    #[test]
    fn u32_info_preserves_non_negative_i32_repr() {
        assert_eq!(
            <u32 as QtType>::INFO.repr(),
            QtValueRepr::I32 { non_negative: true }
        );
        assert!(<u32 as QtType>::INFO.is_non_negative());
    }

    #[test]
    fn enum_info_exposes_declared_domain() {
        let info = <FlexDirection as QtType>::INFO;
        assert_eq!(info.enum_meta(), Some(FlexDirection::META));
        assert_eq!(info.repr(), QtValueRepr::Enum(FlexDirection::META));
    }

    #[test]
    fn accepts_qt_value_follows_repr_without_probing() {
        let info = <String as QtType>::INFO;
        assert!(info.accepts_qt_value(&QtValue::String(String::new())));
        assert!(!info.accepts_qt_value(&QtValue::Bool(false)));
    }

    #[test]
    fn merged_props_overlay_manual_decl_on_exported_leaf() {
        static ROOT_PROPS: SpecPropTree = SpecPropTree {
            type_name: "TestProps",
            nodes: &[super::SpecPropNode::Leaf(super::SpecLeafProp {
                rust_name: "text",
                js_name: "text",
                optional: true,
                exported: true,
                default: SpecPropDefaultValue::None,
                value_type: <String as QtType>::INFO,
                lowering: super::PropLowering::MetaProperty("text"),
                read_lowering: None,
                behavior: super::PropBehavior::State,
            })],
        };
        static METHODS: super::SpecMethodSet = <super::NoMethods as QtMethodSet>::SPEC;
        static EVENTS: super::SpecEventSet = super::SpecEventSet {
            uses: &[],
            events: &[],
        };
        static WIDGET: SpecWidgetBinding = SpecWidgetBinding {
            spec_key: crate::decl::SpecWidgetKey::new("test::Widget"),
            kind_name: "test",
            type_name: "TestWidget",
            children: super::ChildrenKind::None,
            host: HostMeta::root(),
            default_layout: None,
            props: &ROOT_PROPS,
            events: &EVENTS,
            methods: &METHODS,
        };
        static MANUAL: crate::runtime::WidgetPropDecl = crate::runtime::WidgetPropDecl {
            spec_key: crate::decl::SpecWidgetKey::new("test::Widget"),
            create_instance: None,
            create_props: &[],
            props: &[SpecPropDecl {
                path: &["text"],
                value_type: <String as QtType>::INFO,
                default: SpecPropDefaultValue::String("manual"),
                init_setter_slot: Some(7),
                setter_slot: Some(11),
                getter_slot: Some(3),
            }],
        };

        let merged = merged_props(&WIDGET, Some(&MANUAL));

        assert_eq!(
            merged,
            vec![MergedProp {
                path: vec!["text"],
                key: "text".to_owned(),
                value_type: <String as QtType>::INFO,
                is_bound: true,
                default: SpecPropDefaultValue::String("manual"),
                init_setter_slot: Some(7),
                setter_slot: Some(11),
                getter_slot: Some(3),
            }]
        );
        assert_eq!(exported_props(&ROOT_PROPS).len(), 1);
    }

    #[test]
    fn merged_widget_props_merges_duplicate_leaf_specs() {
        static CORE_PROPS: SpecPropTree = SpecPropTree {
            type_name: "TestProps",
            nodes: &[super::SpecPropNode::Leaf(super::SpecLeafProp {
                rust_name: "title",
                js_name: "title",
                optional: false,
                exported: true,
                default: SpecPropDefaultValue::None,
                value_type: <String as QtType>::INFO,
                lowering: super::PropLowering::Custom("title"),
                read_lowering: None,
                behavior: super::PropBehavior::State,
            })],
        };
        static HOST_PROPS: [super::SpecLeafProp; 1] = [super::SpecLeafProp {
            rust_name: "title",
            js_name: "title",
            optional: true,
            exported: true,
            default: SpecPropDefaultValue::String("fallback"),
            value_type: <String as QtType>::INFO,
            lowering: super::PropLowering::Custom("title"),
            read_lowering: Some(super::PropLowering::Custom("title")),
            behavior: super::PropBehavior::State,
        }];

        let merged = super::merged_widget_props("TestWidget", &CORE_PROPS, &HOST_PROPS);
        let [super::SpecPropNode::Leaf(prop)] = merged.nodes else {
            panic!("expected single merged prop");
        };

        assert_eq!(prop.js_name, "title");
        assert!(prop.optional);
        assert_eq!(prop.default, SpecPropDefaultValue::String("fallback"));
        assert_eq!(
            prop.read_lowering,
            Some(super::PropLowering::Custom("title"))
        );
    }

    #[test]
    fn merged_prop_write_mode_prefers_bound_then_live_then_create_only() {
        let bound = MergedProp {
            path: vec!["text"],
            key: "text".to_owned(),
            value_type: <String as QtType>::INFO,
            is_bound: true,
            default: SpecPropDefaultValue::None,
            init_setter_slot: Some(1),
            setter_slot: Some(2),
            getter_slot: Some(3),
        };
        let live = MergedProp {
            is_bound: false,
            setter_slot: Some(7),
            ..bound.clone()
        };
        let create_only = MergedProp {
            is_bound: false,
            setter_slot: None,
            init_setter_slot: Some(9),
            ..bound.clone()
        };

        assert_eq!(bound.write_mode(), EndpointWriteMode::Bound);
        assert_eq!(live.write_mode(), EndpointWriteMode::Live(7));
        assert_eq!(create_only.write_mode(), EndpointWriteMode::CreateOnly(9));
        assert_eq!(create_only.init_only_slot(), Some(9));
        assert_eq!(live.write_slot(), Some(7));
        assert_eq!(bound.write_slot(), None);
    }

    #[test]
    #[should_panic(expected = "duplicate widget prop text for TestWidget")]
    fn host_spec_builder_rejects_duplicate_prop_across_fragments() {
        static PROP: super::SpecLeafProp = super::SpecLeafProp {
            rust_name: "text",
            js_name: "text",
            optional: false,
            exported: true,
            default: SpecPropDefaultValue::None,
            value_type: <String as QtType>::INFO,
            lowering: super::PropLowering::Custom("text"),
            read_lowering: None,
            behavior: super::PropBehavior::State,
        };
        static DECL: super::HostCapabilitySpecDecl = super::HostCapabilitySpecDecl {
            host: None,
            default_layout: None,
            props: &[PROP],
            events: &[],
            methods: &super::NO_METHODS,
        };

        let mut builder = super::WidgetHostSpecBuilder::default();
        builder.extend("TestWidget", &DECL);
        builder.extend("TestWidget", &DECL);
    }

    #[test]
    #[should_panic(expected = "duplicate host event export onClicked for TestWidget")]
    fn host_spec_builder_rejects_duplicate_event_export_across_fragments() {
        static CLICKED_EVENT: super::SpecEventMeta = super::SpecEventMeta {
            index: 0,
            rust_name: "clicked",
            exports: &["onClicked"],
            payload_kind: EventPayloadKind::Unit,
            payload_type: None,
            payload_fields: &[],
            echoes: &[],
            label: "widget::clicked",
            lowering: EventLowering::QtSignal("clicked"),
        };
        static PRESSED_EVENT: super::SpecEventMeta = super::SpecEventMeta {
            index: 0,
            rust_name: "pressed",
            exports: &["onClicked"],
            payload_kind: EventPayloadKind::Unit,
            payload_type: None,
            payload_fields: &[],
            echoes: &[],
            label: "widget::pressed",
            lowering: EventLowering::QtSignal("pressed"),
        };
        static CLICKED_DECL: super::HostCapabilitySpecDecl = super::HostCapabilitySpecDecl {
            host: None,
            default_layout: None,
            props: &[],
            events: &[CLICKED_EVENT],
            methods: &super::NO_METHODS,
        };
        static PRESSED_DECL: super::HostCapabilitySpecDecl = super::HostCapabilitySpecDecl {
            host: None,
            default_layout: None,
            props: &[],
            events: &[PRESSED_EVENT],
            methods: &super::NO_METHODS,
        };

        let mut builder = super::WidgetHostSpecBuilder::default();
        builder.extend("TestWidget", &CLICKED_DECL);
        builder.extend("TestWidget", &PRESSED_DECL);
    }

    #[test]
    #[should_panic(expected = "echoes missing prop checked")]
    fn validate_event_echoes_rejects_missing_prop_target() {
        let props = vec![super::PropMeta {
            index: 0,
            rust_name: "text",
            path: "text",
            js_name: "text",
            symbol: "text",
            value_type: <String as QtType>::INFO,
            optional: false,
            lowering: super::PropLowering::Custom("text"),
            read_lowering: None,
            behavior: super::PropBehavior::State,
        }];
        let events = vec![EventMeta {
            index: 0,
            rust_name: "toggled",
            exports: &["onToggled"],
            label: "widget::toggled",
            payload_kind: EventPayloadKind::Scalar,
            payload_type: Some(<bool as QtType>::INFO),
            payload_fields: vec![],
            lowering: EventLowering::QtSignal("toggled"),
            echoes: vec![EventEchoMeta {
                prop_js_name: "checked",
                value_path: "",
            }],
        }];

        validate_event_echoes(&props, &events);
    }

    #[test]
    #[should_panic(expected = "echo text type mismatch")]
    fn validate_event_echoes_rejects_type_mismatch() {
        let props = vec![super::PropMeta {
            index: 0,
            rust_name: "text",
            path: "text",
            js_name: "text",
            symbol: "text",
            value_type: <String as QtType>::INFO,
            optional: false,
            lowering: super::PropLowering::Custom("text"),
            read_lowering: None,
            behavior: super::PropBehavior::State,
        }];
        let events = vec![EventMeta {
            index: 0,
            rust_name: "changed",
            exports: &["onChanged"],
            label: "widget::changed",
            payload_kind: EventPayloadKind::Object,
            payload_type: None,
            payload_fields: vec![EventFieldMeta {
                rust_name: "text",
                js_name: "text",
                value_type: <bool as QtType>::INFO,
            }],
            lowering: EventLowering::Custom("changed"),
            echoes: vec![EventEchoMeta {
                prop_js_name: "text",
                value_path: "text",
            }],
        }];

        validate_event_echoes(&props, &events);
    }

    #[test]
    #[should_panic(expected = "echoes invalid payload path missing")]
    fn validate_event_echoes_rejects_invalid_payload_path() {
        let props = vec![super::PropMeta {
            index: 0,
            rust_name: "text",
            path: "text",
            js_name: "text",
            symbol: "text",
            value_type: <String as QtType>::INFO,
            optional: false,
            lowering: super::PropLowering::Custom("text"),
            read_lowering: None,
            behavior: super::PropBehavior::State,
        }];
        let events = vec![EventMeta {
            index: 0,
            rust_name: "changed",
            exports: &["onChanged"],
            label: "widget::changed",
            payload_kind: EventPayloadKind::Object,
            payload_type: None,
            payload_fields: vec![EventFieldMeta {
                rust_name: "text",
                js_name: "text",
                value_type: <String as QtType>::INFO,
            }],
            lowering: EventLowering::Custom("changed"),
            echoes: vec![EventEchoMeta {
                prop_js_name: "text",
                value_path: "missing",
            }],
        }];

        validate_event_echoes(&props, &events);
    }
}

pub fn local_widget_binding(spec: &SpecWidgetBinding) -> WidgetBinding {
    resolve_widget_binding(spec, fallback_widget_type_id(spec.spec_key.raw()))
}

fn root_binding() -> &'static WidgetBinding {
    static ROOT: OnceLock<WidgetBinding> = OnceLock::new();
    ROOT.get_or_init(|| WidgetBinding {
        class: NodeClass::Root,
        spec_key: SpecWidgetKey::new("root"),
        widget_type_id: WidgetTypeId::new(0),
        kind_name: "root",
        type_name: "QtRootNode",
        children: ChildrenKind::Nodes,
        host: HostMeta::root(),
        default_layout: None,
        props: Vec::new(),
        events: Vec::new(),
        methods: &NO_METHODS,
    })
}

#[derive(Debug)]
pub struct WidgetRegistry {
    bindings: Vec<&'static WidgetBinding>,
    spec_bindings: Vec<&'static SpecWidgetBinding>,
    bindings_by_type_id: BTreeMap<WidgetTypeId, &'static WidgetBinding>,
    bindings_by_spec_key: BTreeMap<SpecWidgetKey, &'static WidgetBinding>,
    spec_by_key: BTreeMap<SpecWidgetKey, &'static SpecWidgetBinding>,
    host_tags_by_type_id: BTreeMap<WidgetTypeId, u8>,
    widget_type_ids_by_host_tag: BTreeMap<u8, WidgetTypeId>,
}

impl WidgetRegistry {
    pub fn build(libraries: &[&'static WidgetLibraryBindings]) -> Self {
        let mut bindings = Vec::new();
        let mut spec_bindings = Vec::new();
        let mut bindings_by_type_id = BTreeMap::new();
        let mut bindings_by_spec_key = BTreeMap::new();
        let mut spec_by_key = BTreeMap::new();
        let mut host_tags_by_type_id = BTreeMap::new();
        let mut widget_type_ids_by_host_tag = BTreeMap::new();
        let mut next_widget_type_id = 1u32;
        let mut next_host_tag = 1u16;

        for library in libraries {
            for spec in library.spec_bindings {
                let spec_key = spec.spec_key;
                if bindings_by_spec_key.contains_key(&spec_key) {
                    panic!(
                        "duplicate spec widget key {} for {}",
                        spec_key.raw(),
                        spec.type_name
                    );
                }

                let widget_type_id = WidgetTypeId::new(next_widget_type_id);
                next_widget_type_id += 1;
                let host_tag = u8::try_from(next_host_tag)
                    .expect("widget registry exceeds current host tag range");
                next_host_tag += 1;

                let binding: &'static WidgetBinding =
                    Box::leak(Box::new(resolve_widget_binding(spec, widget_type_id)));
                bindings.push(binding);
                spec_bindings.push(*spec);
                bindings_by_type_id.insert(widget_type_id, binding);
                bindings_by_spec_key.insert(spec_key, binding);
                spec_by_key.insert(spec_key, *spec);
                host_tags_by_type_id.insert(widget_type_id, host_tag);
                widget_type_ids_by_host_tag.insert(host_tag, widget_type_id);
            }
        }

        Self {
            bindings,
            spec_bindings,
            bindings_by_type_id,
            bindings_by_spec_key,
            spec_by_key,
            host_tags_by_type_id,
            widget_type_ids_by_host_tag,
        }
    }

    pub fn bindings(&self) -> &[&'static WidgetBinding] {
        self.bindings.as_slice()
    }

    pub fn spec_bindings(&self) -> &[&'static SpecWidgetBinding] {
        self.spec_bindings.as_slice()
    }

    pub fn binding(&self, widget_type_id: WidgetTypeId) -> &'static WidgetBinding {
        self.bindings_by_type_id
            .get(&widget_type_id)
            .copied()
            .unwrap_or_else(|| {
                panic!(
                    "missing widget binding for WidgetTypeId {}",
                    widget_type_id.raw()
                )
            })
    }

    pub fn binding_by_spec_key(&self, spec_key: SpecWidgetKey) -> &'static WidgetBinding {
        self.binding_by_spec_key_opt(spec_key).unwrap_or_else(|| {
            panic!(
                "missing widget binding for spec widget key {}",
                spec_key.raw()
            )
        })
    }

    pub fn binding_by_spec_key_opt(
        &self,
        spec_key: SpecWidgetKey,
    ) -> Option<&'static WidgetBinding> {
        self.bindings_by_spec_key.get(&spec_key).copied()
    }

    pub fn spec_binding(&self, spec_key: SpecWidgetKey) -> &'static SpecWidgetBinding {
        self.spec_by_key.get(&spec_key).copied().unwrap_or_else(|| {
            panic!(
                "missing spec widget binding for spec widget key {}",
                spec_key.raw()
            )
        })
    }

    pub fn host_tag(&self, widget_type_id: WidgetTypeId) -> u8 {
        self.host_tags_by_type_id
            .get(&widget_type_id)
            .copied()
            .unwrap_or_else(|| panic!("missing host tag for WidgetTypeId {}", widget_type_id.raw()))
    }

    pub fn widget_type_id_from_host_tag(&self, tag: u8) -> Option<WidgetTypeId> {
        self.widget_type_ids_by_host_tag.get(&tag).copied()
    }

    pub fn root_binding(&self) -> &'static WidgetBinding {
        root_binding()
    }

    pub fn binding_for_node_class(&self, class: NodeClass) -> &'static WidgetBinding {
        match class {
            NodeClass::Root => self.root_binding(),
            NodeClass::Widget(widget_type_id) => self.binding(widget_type_id),
        }
    }

    pub fn kind_name_for_node_class(&self, class: NodeClass) -> &'static str {
        self.binding_for_node_class(class).kind_name
    }

    pub fn widget_type_id_for_node_class(&self, class: NodeClass) -> Option<WidgetTypeId> {
        match class {
            NodeClass::Root => None,
            NodeClass::Widget(widget_type_id) => Some(widget_type_id),
        }
    }

    fn visit_unique_event_exports<T>(
        &self,
        mut visitor: impl FnMut(u16, &'static str, EventPayloadKind) -> Option<T>,
    ) -> Option<T> {
        let mut seen = Vec::new();
        let mut next_id = 1u16;

        for binding in self.bindings() {
            for event in &binding.events {
                for export in event.exports {
                    if seen.iter().any(|name| *name == *export) {
                        continue;
                    }
                    seen.push(*export);
                    if let Some(result) = visitor(next_id, export, event.payload_kind) {
                        return Some(result);
                    }
                    next_id += 1;
                }
            }
        }

        None
    }

    pub fn prop_id(&self, widget_type_id: WidgetTypeId, js_name: &str) -> Option<u16> {
        self.binding(widget_type_id)
            .props
            .iter()
            .find(|prop| prop.js_name == js_name)
            .map(|prop| u16::from(prop.index) + 1)
    }

    pub fn prop_id_for_class(&self, class: NodeClass, js_name: &str) -> Option<u16> {
        match class {
            NodeClass::Root => None,
            NodeClass::Widget(widget_type_id) => self.prop_id(widget_type_id, js_name),
        }
    }

    pub fn prop_id_by_symbol(&self, symbol: &str) -> Option<u16> {
        self.bindings().iter().find_map(|binding| {
            binding
                .props
                .iter()
                .find(|prop| prop.symbol == symbol)
                .map(|prop| u16::from(prop.index) + 1)
        })
    }

    pub fn prop_meta_for_id(
        &self,
        widget_type_id: WidgetTypeId,
        prop_id_value: u16,
    ) -> Option<&'static PropMeta> {
        let index = usize::from(prop_id_value.checked_sub(1)?);
        self.binding(widget_type_id).props.get(index)
    }

    pub fn prop_meta_for_class_id(
        &self,
        class: NodeClass,
        prop_id_value: u16,
    ) -> Option<&'static PropMeta> {
        match class {
            NodeClass::Root => None,
            NodeClass::Widget(widget_type_id) => {
                self.prop_meta_for_id(widget_type_id, prop_id_value)
            }
        }
    }

    pub fn widget_supports_prop(&self, widget_type_id: WidgetTypeId, js_name: &str) -> bool {
        self.binding(widget_type_id)
            .props
            .iter()
            .any(|prop| prop.js_name == js_name)
    }

    pub fn export_id(&self, export_name: &str) -> Option<u16> {
        self.visit_unique_event_exports(|export_id_value, export, _payload_kind| {
            (export == export_name).then_some(export_id_value)
        })
    }

    pub fn export_meta_for_id(
        &self,
        export_id_value: u16,
    ) -> Option<(&'static str, EventPayloadKind)> {
        self.visit_unique_event_exports(|candidate_id, export, payload_kind| {
            (candidate_id == export_id_value).then_some((export, payload_kind))
        })
    }
}
