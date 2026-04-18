use std::{ffi::CStr, marker::PhantomData, sync::OnceLock};

use napi::{
    Result,
    bindgen_prelude::{FnArgs, Function, JsObjectValue, JsValuesTupleIntoVec, Object},
};

use crate::{
    core_widgets::{self, LabelWidget},
    schema::{QtWidgetDecl, WidgetRegistry},
    widgets::{self, SpinTriangleWidget},
};

const CREATE_WIDGET_METHOD: &CStr = c"__qtSolidCreateWidget";
const APPLY_STRING_PROP_METHOD: &CStr = c"__qtSolidApplyStringProp";
const APPLY_BOOL_PROP_METHOD: &CStr = c"__qtSolidApplyBoolProp";
const APPLY_I32_PROP_METHOD: &CStr = c"__qtSolidApplyI32Prop";
const APPLY_F64_PROP_METHOD: &CStr = c"__qtSolidApplyF64Prop";

#[derive(Clone, Copy)]
struct PropId<T> {
    raw: u16,
    _marker: PhantomData<fn(T) -> T>,
}

impl<T> PropId<T> {
    const fn new(raw: u16) -> Self {
        Self {
            raw,
            _marker: PhantomData,
        }
    }

    const fn raw(self) -> u16 {
        self.raw
    }
}

fn cached_prop_id<Widget, T>(slot: &'static OnceLock<u16>, js_name: &str) -> PropId<T>
where
    Widget: QtWidgetDecl,
{
    PropId::new(*slot.get_or_init(|| {
        let registry = plugin_widget_registry();
        let widget_type_id = registry
            .binding_by_spec_key(Widget::spec().spec_key)
            .widget_type_id;
        registry
            .prop_id(widget_type_id, js_name)
            .unwrap_or_else(|| {
                panic!(
                    "missing prop {} for widget {}",
                    js_name,
                    Widget::spec().spec_key.raw()
                )
            })
    }))
}

fn get_object_method<'env, Args, Return>(
    object: Object<'env>,
    name: &CStr,
) -> Result<Function<'env, Args, Return>>
where
    Args: JsValuesTupleIntoVec,
    Return: napi::bindgen_prelude::FromNapiValue,
{
    object.get_c_named_property_unchecked(name)
}

struct QtAppBridge<'env> {
    app: Object<'env>,
    create_widget: Function<'env, FnArgs<(String,)>, Object<'env>>,
}

impl<'env> QtAppBridge<'env> {
    fn new(app: Object<'env>) -> Result<Self> {
        Ok(Self {
            app,
            create_widget: get_object_method(app, CREATE_WIDGET_METHOD)?,
        })
    }

    fn create_widget<T: QtWidgetDecl>(&self) -> Result<Object<'env>> {
        self.create_widget.apply(
            self.app,
            FnArgs::from((T::spec().spec_key.raw().to_owned(),)),
        )
    }
}

struct QtNodeBridge<'env> {
    node: Object<'env>,
}

impl<'env> QtNodeBridge<'env> {
    fn new(node: Object<'env>) -> Self {
        Self { node }
    }

    fn apply<T: QtBridgeValue<'env>>(&self, prop_id: PropId<T>, value: T) -> Result<()> {
        T::apply(self.node, prop_id.raw(), value)
    }
}

trait QtBridgeValue<'env>: Sized {
    fn apply(node: Object<'env>, prop_id: u16, value: Self) -> Result<()>;
}

impl<'env> QtBridgeValue<'env> for String {
    fn apply(node: Object<'env>, prop_id: u16, value: Self) -> Result<()> {
        let apply: Function<'env, FnArgs<(u16, String)>, ()> =
            get_object_method(node, APPLY_STRING_PROP_METHOD)?;
        apply.apply(node, FnArgs::from((prop_id, value)))
    }
}

impl<'env> QtBridgeValue<'env> for bool {
    fn apply(node: Object<'env>, prop_id: u16, value: Self) -> Result<()> {
        let apply: Function<'env, FnArgs<(u16, bool)>, ()> =
            get_object_method(node, APPLY_BOOL_PROP_METHOD)?;
        apply.apply(node, FnArgs::from((prop_id, value)))
    }
}

impl<'env> QtBridgeValue<'env> for i32 {
    fn apply(node: Object<'env>, prop_id: u16, value: Self) -> Result<()> {
        let apply: Function<'env, FnArgs<(u16, i32)>, ()> =
            get_object_method(node, APPLY_I32_PROP_METHOD)?;
        apply.apply(node, FnArgs::from((prop_id, value)))
    }
}

impl<'env> QtBridgeValue<'env> for f64 {
    fn apply(node: Object<'env>, prop_id: u16, value: Self) -> Result<()> {
        let apply: Function<'env, FnArgs<(u16, f64)>, ()> =
            get_object_method(node, APPLY_F64_PROP_METHOD)?;
        apply.apply(node, FnArgs::from((prop_id, value)))
    }
}

fn banner_prop_id<T>(slot: &'static OnceLock<u16>, js_name: &str) -> PropId<T> {
    cached_prop_id::<LabelWidget, T>(slot, js_name)
}

fn plugin_widget_registry() -> &'static WidgetRegistry {
    static REGISTRY: OnceLock<WidgetRegistry> = OnceLock::new();
    REGISTRY.get_or_init(|| {
        WidgetRegistry::build(&[
            core_widgets::core_widgets_library(),
            widgets::example_widgets_library(),
        ])
    })
}

fn banner_text_prop_id() -> PropId<String> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "text")
}

fn banner_enabled_prop_id() -> PropId<bool> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "enabled")
}

fn banner_width_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "width")
}

fn banner_height_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "height")
}

fn banner_grow_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "grow")
}

fn banner_min_width_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "minWidth")
}

fn banner_min_height_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "minHeight")
}

fn banner_font_family_prop_id() -> PropId<String> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "family")
}

fn banner_font_point_size_prop_id() -> PropId<f64> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "pointSize")
}

fn banner_shrink_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "shrink")
}

fn banner_font_weight_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "weight")
}

fn banner_font_italic_prop_id() -> PropId<bool> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    banner_prop_id(&SLOT, "italic")
}

fn spin_triangle_prop_id<T>(slot: &'static OnceLock<u16>, js_name: &str) -> PropId<T> {
    cached_prop_id::<SpinTriangleWidget, T>(slot, js_name)
}

fn spin_triangle_width_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    spin_triangle_prop_id(&SLOT, "width")
}

fn spin_triangle_enabled_prop_id() -> PropId<bool> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    spin_triangle_prop_id(&SLOT, "enabled")
}

fn spin_triangle_height_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    spin_triangle_prop_id(&SLOT, "height")
}

fn spin_triangle_min_width_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    spin_triangle_prop_id(&SLOT, "minWidth")
}

fn spin_triangle_min_height_prop_id() -> PropId<i32> {
    static SLOT: OnceLock<u16> = OnceLock::new();
    spin_triangle_prop_id(&SLOT, "minHeight")
}

#[napi_derive::napi]
pub fn create_banner_node(app: Object<'_>) -> Result<Object<'_>> {
    QtAppBridge::new(app)?.create_widget::<LabelWidget>()
}

#[napi_derive::napi]
pub fn apply_banner_text(node: Object<'_>, value: String) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_text_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_enabled(node: Object<'_>, value: bool) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_enabled_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_width(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_width_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_height(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_height_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_grow(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_grow_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_min_width(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_min_width_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_min_height(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_min_height_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_family(node: Object<'_>, value: String) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_font_family_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_point_size(node: Object<'_>, value: f64) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_font_point_size_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_shrink(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_shrink_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_weight(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_font_weight_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_banner_italic(node: Object<'_>, value: bool) -> Result<()> {
    QtNodeBridge::new(node).apply(banner_font_italic_prop_id(), value)
}

#[napi_derive::napi]
pub fn create_spin_triangle_node(app: Object<'_>) -> Result<Object<'_>> {
    QtAppBridge::new(app)?.create_widget::<SpinTriangleWidget>()
}

#[napi_derive::napi]
pub fn apply_spin_triangle_width(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(spin_triangle_width_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_spin_triangle_enabled(node: Object<'_>, value: bool) -> Result<()> {
    QtNodeBridge::new(node).apply(spin_triangle_enabled_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_spin_triangle_height(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(spin_triangle_height_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_spin_triangle_min_width(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(spin_triangle_min_width_prop_id(), value)
}

#[napi_derive::napi]
pub fn apply_spin_triangle_min_height(node: Object<'_>, value: i32) -> Result<()> {
    QtNodeBridge::new(node).apply(spin_triangle_min_height_prop_id(), value)
}

#[cfg(test)]
mod tests {
    use super::{
        banner_enabled_prop_id, banner_font_point_size_prop_id, banner_text_prop_id,
        banner_width_prop_id, plugin_widget_registry,
    };
    use crate::QtWidgetDecl;
    use crate::core_widgets::LabelWidget;

    #[test]
    fn typed_banner_prop_ids_match_expected_raw_ids() {
        let registry = plugin_widget_registry();
        let widget_type_id = registry
            .binding_by_spec_key(LabelWidget::spec().spec_key)
            .widget_type_id;
        assert_eq!(
            banner_text_prop_id().raw(),
            registry
                .prop_id(widget_type_id, "text")
                .expect("text prop id")
        );
        assert_eq!(
            banner_enabled_prop_id().raw(),
            registry
                .prop_id(widget_type_id, "enabled")
                .expect("enabled prop id")
        );
        assert_eq!(
            banner_width_prop_id().raw(),
            registry
                .prop_id(widget_type_id, "width")
                .expect("width prop id")
        );
        assert_eq!(
            banner_font_point_size_prop_id().raw(),
            registry
                .prop_id(widget_type_id, "pointSize")
                .expect("point size prop id")
        );
    }
}
