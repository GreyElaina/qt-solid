use std::{
    collections::BTreeMap,
    error::Error,
    fmt,
    marker::PhantomData,
    pin::Pin,
    sync::{Arc, Mutex},
};

use crate::{decl::SpecWidgetKey, vello::PaintSceneFrame, vello::peniko::color::PremulRgba8};

pub use linkme;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WidgetError {
    message: String,
}

impl WidgetError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
        }
    }

    pub fn unsupported_paint_device(device_name: &str) -> Self {
        Self::new(format!("unsupported paint device: {device_name}"))
    }

    pub fn message(&self) -> &str {
        &self.message
    }

    pub fn is_unsupported_paint_device(&self) -> bool {
        self.message.starts_with("unsupported paint device: ")
    }
}

impl fmt::Display for WidgetError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl Error for WidgetError {}

pub type WidgetResult<T> = Result<T, WidgetError>;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtOpaqueBorrow {
    Ref,
    Mut,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QtOpaqueInfo {
    rust_path: &'static str,
    cxx_class: &'static str,
    cxx_include: &'static str,
    borrow: QtOpaqueBorrow,
}

impl QtOpaqueInfo {
    pub const fn new(
        rust_path: &'static str,
        cxx_class: &'static str,
        cxx_include: &'static str,
        borrow: QtOpaqueBorrow,
    ) -> Self {
        Self {
            rust_path,
            cxx_class,
            cxx_include,
            borrow,
        }
    }

    pub const fn rust_path(self) -> &'static str {
        self.rust_path
    }

    pub const fn cxx_class(self) -> &'static str {
        self.cxx_class
    }

    pub const fn cxx_include(self) -> &'static str {
        self.cxx_include
    }

    pub const fn borrow(self) -> QtOpaqueBorrow {
        self.borrow
    }

    pub fn matches_host(self, actual: Self) -> bool {
        self.cxx_class == actual.cxx_class
            && self.cxx_include == actual.cxx_include
            && self.borrow == actual.borrow
    }
}

/// Internal bridge carrier for runtime dispatch.
/// Front-door APIs should prefer `IntoQt` / `TryFromQt`.
#[derive(Debug, Clone, PartialEq)]
pub enum QtValue {
    Unit,
    String(String),
    Bool(bool),
    I32(i32),
    F64(f64),
    Enum(i32),
    Color(QtColor),
    Point(QtPoint),
    Size(QtSize),
    Rect(QtRect),
    Affine(QtAffine),
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetCreateProp {
    pub key: String,
    pub value: QtValue,
}

pub fn parse_widget_create_prop<T>(props: &[WidgetCreateProp], key: &str) -> WidgetResult<Option<T>>
where
    T: crate::schema::QtType + TryFromQt,
{
    let Some(prop) = props.iter().find(|prop| prop.key == key) else {
        return Ok(None);
    };
    T::try_from_qt(prop.value.clone()).map(Some)
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QtColor {
    pub red: f32,
    pub green: f32,
    pub blue: f32,
    pub alpha: f32,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QtPoint {
    pub x: f64,
    pub y: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QtSize {
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QtRect {
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QtAffine {
    pub xx: f64,
    pub xy: f64,
    pub yx: f64,
    pub yy: f64,
    pub dx: f64,
    pub dy: f64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WidgetCaptureFormat {
    Argb32Premultiplied,
    Rgba8Premultiplied,
}

impl WidgetCaptureFormat {
    pub const fn bytes_per_pixel(self) -> usize {
        match self {
            Self::Argb32Premultiplied => 4,
            Self::Rgba8Premultiplied => 4,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct WidgetCapture {
    format: WidgetCaptureFormat,
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    bytes: Vec<u8>,
}

impl WidgetCapture {
    fn validate_layout(
        format: WidgetCaptureFormat,
        width_px: u32,
        height_px: u32,
        stride: usize,
    ) -> WidgetResult<usize> {
        let bytes_per_pixel = format.bytes_per_pixel();
        if stride % bytes_per_pixel != 0 {
            return Err(WidgetError::new(format!(
                "widget capture stride {stride} is not aligned to pixel size {bytes_per_pixel}"
            )));
        }

        let min_stride = width_px as usize * bytes_per_pixel;
        if width_px > 0 && stride < min_stride {
            return Err(WidgetError::new(format!(
                "widget capture stride {stride} is smaller than minimum {min_stride}"
            )));
        }

        stride
            .checked_mul(height_px as usize)
            .ok_or_else(|| WidgetError::new("widget capture buffer length overflow"))
    }

    pub fn new_zeroed(
        format: WidgetCaptureFormat,
        width_px: u32,
        height_px: u32,
        stride: usize,
        scale_factor: f64,
    ) -> WidgetResult<Self> {
        let byte_len = Self::validate_layout(format, width_px, height_px, stride)?;

        Ok(Self {
            format,
            width_px,
            height_px,
            stride,
            scale_factor,
            bytes: vec![0; byte_len],
        })
    }

    pub fn from_premul_rgba_pixels(
        width_px: u32,
        height_px: u32,
        stride: usize,
        scale_factor: f64,
        pixels: Vec<PremulRgba8>,
    ) -> WidgetResult<Self> {
        let format = WidgetCaptureFormat::Rgba8Premultiplied;
        let byte_len = Self::validate_layout(format, width_px, height_px, stride)?;
        let mut bytes = Vec::with_capacity(pixels.len() * 4);
        for pixel in pixels {
            bytes.extend_from_slice(&[pixel.r, pixel.g, pixel.b, pixel.a]);
        }
        if bytes.len() != byte_len {
            return Err(WidgetError::new(format!(
                "widget capture byte length {} does not match expected {byte_len}",
                bytes.len()
            )));
        }

        Ok(Self {
            format,
            width_px,
            height_px,
            stride,
            scale_factor,
            bytes,
        })
    }

    pub fn format(&self) -> WidgetCaptureFormat {
        self.format
    }

    pub fn width_px(&self) -> u32 {
        self.width_px
    }

    pub fn height_px(&self) -> u32 {
        self.height_px
    }

    pub fn stride(&self) -> usize {
        self.stride
    }

    pub fn scale_factor(&self) -> f64 {
        self.scale_factor
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn bytes_mut(&mut self) -> &mut [u8] {
        &mut self.bytes
    }

    pub fn into_bytes(self) -> Vec<u8> {
        self.bytes
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub struct NonNegativeF64(pub f64);

impl NonNegativeF64 {
    pub fn new(value: f64) -> WidgetResult<Self> {
        if value < 0.0 {
            return Err(WidgetError::new(format!(
                "expected non-negative f64, got {value}"
            )));
        }
        Ok(Self(value))
    }

    pub fn get(self) -> f64 {
        self.0
    }
}

#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct FontWeight(pub u16);

impl FontWeight {
    pub const MAX: u16 = 1000;

    pub fn new(value: u32) -> WidgetResult<Self> {
        if value > u32::from(Self::MAX) {
            return Err(WidgetError::new(format!(
                "expected font weight in 0..={}, got {value}",
                Self::MAX
            )));
        }
        Ok(Self(value as u16))
    }

    pub fn get(self) -> u16 {
        self.0
    }
}

pub trait QtTypeName {
    fn qt_type_name() -> &'static str;
}

pub trait IntoQt: QtTypeName {
    fn into_qt(self) -> WidgetResult<QtValue>;
}

pub trait TryFromQt: QtTypeName + Sized {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self>;
}

pub trait QtOpaqueFacade {
    const INFO: QtOpaqueInfo;
}

pub trait QtOpaqueHostDyn {
    fn opaque_info(&self) -> QtOpaqueInfo;
}

pub trait QtOpaqueHostRefDyn: QtOpaqueHostDyn {
    fn call_host_slot(&self, slot: u16, args: &[QtValue]) -> WidgetResult<QtValue>;
}

pub trait QtOpaqueHostMutDyn: QtOpaqueHostDyn {
    fn call_host_slot_mut(&mut self, slot: u16, args: &[QtValue]) -> WidgetResult<QtValue>;
}

pub trait QtHostMethodOwner {
    fn __qt_call_host_method(
        &self,
        slot: u16,
        name: &str,
        args: Vec<QtValue>,
    ) -> WidgetResult<QtValue>;
}

pub trait QtHostMethodOwnerMut {
    fn __qt_call_host_method_mut(
        &mut self,
        slot: u16,
        name: &str,
        args: Vec<QtValue>,
    ) -> WidgetResult<QtValue>;
}

impl<T> QtHostMethodOwnerMut for T
where
    T: QtHostMethodOwner + ?Sized,
{
    fn __qt_call_host_method_mut(
        &mut self,
        slot: u16,
        name: &str,
        args: Vec<QtValue>,
    ) -> WidgetResult<QtValue> {
        self.__qt_call_host_method(slot, name, args)
    }
}

pub trait QtEnumValue: crate::schema::QtEnumDomain + Copy + Sized + QtTypeName {
    fn into_qt_enum(self) -> i32;

    fn try_from_qt_enum(value: i32) -> WidgetResult<Self>;
}

#[derive(Debug)]
pub struct QtOpaqueRef<'a, T: ?Sized> {
    inner: &'a T,
}

impl<'a, T: ?Sized> QtOpaqueRef<'a, T> {
    pub fn new(inner: &'a T) -> Self {
        Self { inner }
    }

    pub fn get(&self) -> &'a T {
        self.inner
    }
}

#[derive(Debug)]
pub struct QtOpaqueMut<'a, T: ?Sized> {
    inner: Pin<&'a mut T>,
}

impl<'a, T: ?Sized> QtOpaqueMut<'a, T> {
    pub fn new(inner: Pin<&'a mut T>) -> Self {
        Self { inner }
    }

    pub fn get_mut(&mut self) -> Pin<&mut T> {
        self.inner.as_mut()
    }
}

impl QtTypeName for () {
    fn qt_type_name() -> &'static str {
        "unit"
    }
}

impl IntoQt for () {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::Unit)
    }
}

impl TryFromQt for () {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::Unit => Ok(()),
            _ => Err(WidgetError::new("expected Qt unit value")),
        }
    }
}

impl QtTypeName for String {
    fn qt_type_name() -> &'static str {
        "string"
    }
}

impl IntoQt for String {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::String(self))
    }
}

impl TryFromQt for String {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::String(value) => Ok(value),
            _ => Err(WidgetError::new("expected Qt string value")),
        }
    }
}

impl QtTypeName for &str {
    fn qt_type_name() -> &'static str {
        "string"
    }
}

impl IntoQt for &str {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::String(self.to_owned()))
    }
}

impl QtTypeName for bool {
    fn qt_type_name() -> &'static str {
        "boolean"
    }
}

impl IntoQt for bool {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::Bool(self))
    }
}

impl TryFromQt for bool {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::Bool(value) => Ok(value),
            _ => Err(WidgetError::new("expected Qt boolean value")),
        }
    }
}

impl QtTypeName for i32 {
    fn qt_type_name() -> &'static str {
        "i32"
    }
}

impl IntoQt for i32 {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::I32(self))
    }
}

impl TryFromQt for i32 {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::I32(value) => Ok(value),
            _ => Err(WidgetError::new("expected Qt i32 value")),
        }
    }
}

impl QtTypeName for u32 {
    fn qt_type_name() -> &'static str {
        "u32"
    }
}

impl IntoQt for u32 {
    fn into_qt(self) -> WidgetResult<QtValue> {
        let value = i32::try_from(self)
            .map_err(|_| WidgetError::new(format!("u32 value {self} exceeds Qt i32 range")))?;
        Ok(QtValue::I32(value))
    }
}

impl TryFromQt for u32 {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::I32(value) => u32::try_from(value)
                .map_err(|_| WidgetError::new(format!("expected non-negative i32, got {value}"))),
            _ => Err(WidgetError::new("expected Qt i32 value")),
        }
    }
}

impl QtTypeName for f64 {
    fn qt_type_name() -> &'static str {
        "f64"
    }
}

impl IntoQt for f64 {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::F64(self))
    }
}

impl TryFromQt for f64 {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::F64(value) => Ok(value),
            _ => Err(WidgetError::new("expected Qt f64 value")),
        }
    }
}

impl QtTypeName for NonNegativeF64 {
    fn qt_type_name() -> &'static str {
        "NonNegativeF64"
    }
}

impl QtTypeName for FontWeight {
    fn qt_type_name() -> &'static str {
        "FontWeight"
    }
}

impl IntoQt for NonNegativeF64 {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::F64(self.0))
    }
}

impl TryFromQt for NonNegativeF64 {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::F64(value) => NonNegativeF64::new(value),
            _ => Err(WidgetError::new("expected Qt f64 value")),
        }
    }
}

impl IntoQt for FontWeight {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::I32(i32::from(self.0)))
    }
}

impl TryFromQt for FontWeight {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::I32(value) => {
                let value = u32::try_from(value).map_err(|_| {
                    WidgetError::new(format!("expected non-negative i32, got {value}"))
                })?;
                FontWeight::new(value)
            }
            _ => Err(WidgetError::new("expected Qt i32 value")),
        }
    }
}

impl<T> IntoQt for T
where
    T: QtEnumValue,
{
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::Enum(self.into_qt_enum()))
    }
}

impl<T> TryFromQt for T
where
    T: QtEnumValue,
{
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::Enum(value) => T::try_from_qt_enum(value),
            _ => Err(WidgetError::new(format!(
                "expected Qt enum value for {}",
                T::qt_type_name()
            ))),
        }
    }
}

impl QtTypeName for QtColor {
    fn qt_type_name() -> &'static str {
        "QtColor"
    }
}

impl IntoQt for QtColor {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::Color(self))
    }
}

impl TryFromQt for QtColor {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::Color(value) => Ok(value),
            _ => Err(WidgetError::new("expected QtColor value")),
        }
    }
}

impl QtTypeName for QtPoint {
    fn qt_type_name() -> &'static str {
        "QtPoint"
    }
}

impl IntoQt for QtPoint {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::Point(self))
    }
}

impl TryFromQt for QtPoint {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::Point(value) => Ok(value),
            _ => Err(WidgetError::new("expected QtPoint value")),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    };

    use super::{
        HostBehaviorRuntimeDecl, Paint, PaintDevice, QtOpaqueBorrow, QtOpaqueHookDispatchSurface,
        QtOpaqueHostDyn, QtOpaqueHostMutDyn, QtOpaqueInfo, QtValue, WidgetCapture,
        WidgetCaptureFormat, WidgetHandle, WidgetHandleOwner, WidgetPaintRuntimeMeta,
        WidgetRuntimeHandle, new_widget_instance,
    };
    use crate::vello::{FrameTime, PaintSceneFrame, Scene};

    #[test]
    fn opaque_host_match_ignores_rust_path() {
        let spec = QtOpaqueInfo::new(
            "core_widgets::QtPainter",
            "QPainter",
            "<QtGui/QPainter>",
            QtOpaqueBorrow::Mut,
        );
        let host = QtOpaqueInfo::new(
            "native::uv_host::ffi::QPainter",
            "QPainter",
            "<QtGui/QPainter>",
            QtOpaqueBorrow::Mut,
        );

        assert!(spec.matches_host(host));
    }

    #[test]
    fn opaque_host_match_rejects_different_contract() {
        let expected = QtOpaqueInfo::new(
            "core_widgets::QtPainter",
            "QPainter",
            "<QtGui/QPainter>",
            QtOpaqueBorrow::Mut,
        );
        let wrong_borrow = QtOpaqueInfo::new(
            "native::uv_host::ffi::QPainter",
            "QPainter",
            "<QtGui/QPainter>",
            QtOpaqueBorrow::Ref,
        );
        let wrong_include = QtOpaqueInfo::new(
            "native::uv_host::ffi::QPainter",
            "QPainter",
            "<QtWidgets/QPainter>",
            QtOpaqueBorrow::Mut,
        );
        let wrong_class = QtOpaqueInfo::new(
            "native::uv_host::ffi::QWidget",
            "QWidget",
            "<QtWidgets/QWidget>",
            QtOpaqueBorrow::Mut,
        );

        assert!(!expected.matches_host(wrong_borrow));
        assert!(!expected.matches_host(wrong_include));
        assert!(!expected.matches_host(wrong_class));
    }

    struct DummyRuntimeHandle;

    impl WidgetRuntimeHandle for DummyRuntimeHandle {
        fn apply_prop_path(&self, _path: &str, _value: QtValue) -> super::WidgetResult<()> {
            Ok(())
        }

        fn call_host_method(&self, _name: &str, _args: &[QtValue]) -> super::WidgetResult<QtValue> {
            Ok(QtValue::Unit)
        }

        fn request_repaint(&self) -> super::WidgetResult<()> {
            Ok(())
        }

        fn capture(&self) -> super::WidgetResult<WidgetCapture> {
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 4, 3, 16, 2.0)
        }
    }

    struct DummyOpaqueHost;

    impl QtOpaqueHostDyn for DummyOpaqueHost {
        fn opaque_info(&self) -> QtOpaqueInfo {
            QtOpaqueInfo::new(
                "tests::DummyOpaque",
                "DummyOpaque",
                "<DummyOpaque>",
                QtOpaqueBorrow::Mut,
            )
        }
    }

    impl QtOpaqueHostMutDyn for DummyOpaqueHost {
        fn call_host_slot_mut(
            &mut self,
            _slot: u16,
            _args: &[QtValue],
        ) -> super::WidgetResult<QtValue> {
            Ok(QtValue::Unit)
        }
    }

    struct DummyWidget {
        handle: WidgetHandle,
        call_count: usize,
        observed_count: Arc<AtomicUsize>,
    }

    impl WidgetHandleOwner for DummyWidget {
        fn widget_handle(&self) -> WidgetHandle {
            self.handle.clone()
        }
    }

    struct DummyMethods;

    impl QtOpaqueHookDispatchSurface<DummyWidget> for DummyMethods {
        fn __qt_invoke_opaque_hook(
            widget: &mut DummyWidget,
            hook_name: &str,
            _host: &mut dyn QtOpaqueHostMutDyn,
        ) -> super::WidgetResult<()> {
            match hook_name {
                "tick" => {
                    widget.call_count += 1;
                    widget
                        .observed_count
                        .store(widget.call_count, Ordering::SeqCst);
                    Ok(())
                }
                _ => Err(super::WidgetError::new(format!(
                    "unknown opaque hook {hook_name}"
                ))),
            }
        }
    }

    impl Paint<PaintSceneFrame<'_>> for DummyWidget {
        fn paint(&mut self, frame: &mut PaintSceneFrame<'_>) {
            let _ = frame.scene();
            self.call_count += 1;
            self.observed_count.store(self.call_count, Ordering::SeqCst);
            frame.request_next_frame();
        }
    }

    unsafe fn dummy_widget_paint(raw: *mut (), device: PaintDevice<'_>) -> super::WidgetResult<()> {
        let widget = unsafe { &mut *raw.cast::<DummyWidget>() };
        match device {
            PaintDevice::OpaqueHost(_) => Err(super::WidgetError::new(
                "dummy widget does not support opaque host paint",
            )),
            PaintDevice::Scene(frame) => {
                Paint::paint(widget, frame);
                Ok(())
            }
        }
    }

    const DUMMY_WIDGET_PAINT_DECL: HostBehaviorRuntimeDecl = HostBehaviorRuntimeDecl {
        host_events: &super::NO_WIDGET_HOST_EVENT_RUNTIME,
        host_overrides: &super::NO_WIDGET_HOST_OVERRIDE_RUNTIME,
        paint: Some(WidgetPaintRuntimeMeta {
            rust_name: "paint",
            invoke: dummy_widget_paint,
        }),
    };

    #[test]
    fn widget_instance_preserves_state_across_opaque_hook_dispatch() {
        let observed_count = Arc::new(AtomicUsize::new(0));
        let handle = WidgetHandle::new(DummyRuntimeHandle);
        let instance = new_widget_instance::<DummyWidget, DummyMethods>(
            handle.clone(),
            DummyWidget {
                handle,
                call_count: 0,
                observed_count: Arc::clone(&observed_count),
            },
            &DUMMY_WIDGET_PAINT_DECL,
            &super::NO_WIDGET_PROP_RUNTIME_DECL,
        );
        let mut host = DummyOpaqueHost;

        instance
            .invoke_opaque_hook("tick", &mut host)
            .expect("first opaque hook dispatch");
        instance
            .invoke_opaque_hook("tick", &mut host)
            .expect("second opaque hook dispatch");

        assert_eq!(observed_count.load(Ordering::SeqCst), 2);
    }

    #[test]
    fn widget_instance_without_opaque_hooks_rejects_dispatch() {
        let handle = WidgetHandle::new(DummyRuntimeHandle);
        let instance = new_widget_instance::<DummyWidget, crate::schema::NoMethods>(
            handle.clone(),
            DummyWidget {
                handle,
                call_count: 0,
                observed_count: Arc::new(AtomicUsize::new(0)),
            },
            &super::NO_HOST_BEHAVIOR_RUNTIME_DECL,
            &super::NO_WIDGET_PROP_RUNTIME_DECL,
        );
        let mut host = DummyOpaqueHost;
        let error = instance
            .invoke_opaque_hook("missing", &mut host)
            .expect_err("missing opaque hook should fail");

        assert_eq!(
            error.message(),
            "widget opaque hook missing is not registered"
        );
    }

    #[test]
    fn widget_instance_dispatches_paint() {
        let observed_count = Arc::new(AtomicUsize::new(0));
        let handle = WidgetHandle::new(DummyRuntimeHandle);
        let instance = new_widget_instance::<DummyWidget, DummyMethods>(
            handle.clone(),
            DummyWidget {
                handle,
                call_count: 0,
                observed_count: Arc::clone(&observed_count),
            },
            &DUMMY_WIDGET_PAINT_DECL,
            &super::NO_WIDGET_PROP_RUNTIME_DECL,
        );

        let mut scene = Scene::new(false);
        let mut next_frame_requested = false;
        let mut dirty_rects = Vec::new();
        let mut frame = PaintSceneFrame::new(
            100.0,
            40.0,
            1.0,
            FrameTime {
                elapsed: std::time::Duration::from_millis(33),
                delta: std::time::Duration::from_millis(16),
            },
            &mut scene,
            &mut next_frame_requested,
            &mut dirty_rects,
        );

        instance
            .paint(PaintDevice::Scene(&mut frame))
            .expect("paint dispatch should succeed");

        assert_eq!(observed_count.load(Ordering::SeqCst), 1);
        assert!(next_frame_requested);
    }

    #[test]
    fn widget_instance_without_paint_decl_rejects_paint() {
        let handle = WidgetHandle::new(DummyRuntimeHandle);
        let instance = new_widget_instance::<DummyWidget, DummyMethods>(
            handle.clone(),
            DummyWidget {
                handle,
                call_count: 0,
                observed_count: Arc::new(AtomicUsize::new(0)),
            },
            &super::NO_HOST_BEHAVIOR_RUNTIME_DECL,
            &super::NO_WIDGET_PROP_RUNTIME_DECL,
        );

        let mut scene = Scene::new(false);
        let mut next_frame_requested = false;
        let mut dirty_rects = Vec::new();
        let mut frame = PaintSceneFrame::new(
            100.0,
            40.0,
            1.0,
            FrameTime {
                elapsed: std::time::Duration::from_millis(33),
                delta: std::time::Duration::from_millis(16),
            },
            &mut scene,
            &mut next_frame_requested,
            &mut dirty_rects,
        );

        let error = instance
            .paint(PaintDevice::Scene(&mut frame))
            .expect_err("paint without host decl should fail");
        assert_eq!(error.message(), "unsupported paint device: scene");
    }

    #[test]
    fn widget_handle_can_request_repaint() {
        let handle = WidgetHandle::new(DummyRuntimeHandle);
        handle
            .request_repaint()
            .expect("request_repaint should succeed");
    }

    #[test]
    fn widget_capture_allocates_expected_bytes() {
        let capture =
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 4, 3, 16, 2.0)
                .expect("capture alloc");

        assert_eq!(capture.bytes().len(), 48);
        assert_eq!(capture.width_px(), 4);
        assert_eq!(capture.height_px(), 3);
        assert_eq!(capture.stride(), 16);
        assert_eq!(capture.scale_factor(), 2.0);
    }

    #[test]
    fn widget_handle_can_capture() {
        let handle = WidgetHandle::new(DummyRuntimeHandle);
        let capture = handle.capture().expect("capture should succeed");

        assert_eq!(capture.width_px(), 4);
        assert_eq!(capture.height_px(), 3);
    }
}

impl QtTypeName for QtSize {
    fn qt_type_name() -> &'static str {
        "QtSize"
    }
}

impl IntoQt for QtSize {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::Size(self))
    }
}

impl TryFromQt for QtSize {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::Size(value) => Ok(value),
            _ => Err(WidgetError::new("expected QtSize value")),
        }
    }
}

impl QtTypeName for QtRect {
    fn qt_type_name() -> &'static str {
        "QtRect"
    }
}

impl IntoQt for QtRect {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::Rect(self))
    }
}

impl TryFromQt for QtRect {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::Rect(value) => Ok(value),
            _ => Err(WidgetError::new("expected QtRect value")),
        }
    }
}

impl QtTypeName for QtAffine {
    fn qt_type_name() -> &'static str {
        "QtAffine"
    }
}

impl IntoQt for QtAffine {
    fn into_qt(self) -> WidgetResult<QtValue> {
        Ok(QtValue::Affine(self))
    }
}

impl TryFromQt for QtAffine {
    fn try_from_qt(value: QtValue) -> WidgetResult<Self> {
        match value {
            QtValue::Affine(value) => Ok(value),
            _ => Err(WidgetError::new("expected QtAffine value")),
        }
    }
}

pub trait WidgetRuntimeHandle: Send + Sync + 'static {
    fn apply_prop_path(&self, path: &str, value: QtValue) -> WidgetResult<()>;

    fn call_host_method(&self, name: &str, args: &[QtValue]) -> WidgetResult<QtValue>;

    fn request_repaint(&self) -> WidgetResult<()>;

    fn capture(&self) -> WidgetResult<WidgetCapture>;
}

#[derive(Clone)]
pub struct WidgetHandle {
    inner: Arc<dyn WidgetRuntimeHandle>,
}

impl WidgetHandle {
    pub fn new(inner: impl WidgetRuntimeHandle) -> Self {
        Self {
            inner: Arc::new(inner),
        }
    }

    pub fn from_arc(inner: Arc<dyn WidgetRuntimeHandle>) -> Self {
        Self { inner }
    }

    pub fn apply_prop_path(&self, path: &str, value: QtValue) -> WidgetResult<()> {
        self.inner.apply_prop_path(path, value)
    }

    pub fn set_prop<T>(&self, path: &str, value: T) -> WidgetResult<()>
    where
        T: IntoQt,
    {
        self.inner.apply_prop_path(path, value.into_qt()?)
    }

    pub fn call_host_method(&self, name: &str, args: &[QtValue]) -> WidgetResult<QtValue> {
        self.inner.call_host_method(name, args)
    }

    pub fn call_host_method_typed<R>(&self, name: &str, args: &[QtValue]) -> WidgetResult<R>
    where
        R: TryFromQt,
    {
        R::try_from_qt(self.inner.call_host_method(name, args)?)
    }

    pub fn request_repaint(&self) -> WidgetResult<()> {
        self.inner.request_repaint()
    }

    pub fn capture(&self) -> WidgetResult<WidgetCapture> {
        self.inner.capture()
    }
}

impl fmt::Debug for WidgetHandle {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetHandle").finish_non_exhaustive()
    }
}

fn widget_storage_key<T: ?Sized>(widget: &T) -> usize {
    (widget as *const T).cast::<()>() as usize
}

fn widget_handle_slots() -> &'static Mutex<BTreeMap<usize, WidgetHandle>> {
    static SLOTS: std::sync::OnceLock<Mutex<BTreeMap<usize, WidgetHandle>>> =
        std::sync::OnceLock::new();
    SLOTS.get_or_init(|| Mutex::new(BTreeMap::new()))
}

pub fn attach_widget_handle<T: ?Sized>(widget: &T, handle: WidgetHandle) {
    let mut slots = widget_handle_slots()
        .lock()
        .expect("widget handle slot mutex poisoned");
    slots.insert(widget_storage_key(widget), handle);
}

pub fn detach_widget_handle<T: ?Sized>(widget: &T) {
    if let Ok(mut slots) = widget_handle_slots().lock() {
        slots.remove(&widget_storage_key(widget));
    }
}

pub fn widget_handle_for<T: ?Sized>(widget: &T) -> WidgetResult<WidgetHandle> {
    let slots = widget_handle_slots()
        .lock()
        .map_err(|_| WidgetError::new("widget handle slot mutex poisoned"))?;
    slots
        .get(&widget_storage_key(widget))
        .cloned()
        .ok_or_else(|| {
            WidgetError::new("widget handle accessed before widget was attached to runtime")
        })
}

pub trait WidgetHandleOwner {
    fn widget_handle(&self) -> WidgetHandle;
}

pub trait Paint<Target> {
    fn paint(&mut self, target: &mut Target);
}

pub enum PaintDevice<'a> {
    OpaqueHost(&'a mut dyn QtOpaqueHostMutDyn),
    Scene(&'a mut PaintSceneFrame<'a>),
}

impl PaintDevice<'_> {
    pub const fn kind_name(&self) -> &'static str {
        match self {
            Self::OpaqueHost(_) => "opaque-host",
            Self::Scene(_) => "scene",
        }
    }
}

pub trait QtWidgetDefaultConstruct: Sized {
    fn __qt_default_construct() -> Self;
}

pub trait QtOpaqueHookDispatchSurface<T> {
    fn __qt_invoke_opaque_hook(
        widget: &mut T,
        hook_name: &str,
        host: &mut dyn QtOpaqueHostMutDyn,
    ) -> WidgetResult<()>;
}

impl<T> QtOpaqueHookDispatchSurface<T> for crate::schema::NoMethods {
    fn __qt_invoke_opaque_hook(
        _widget: &mut T,
        hook_name: &str,
        _host: &mut dyn QtOpaqueHostMutDyn,
    ) -> WidgetResult<()> {
        Err(WidgetError::new(format!(
            "widget opaque hook {hook_name} is not registered"
        )))
    }
}

pub type SpecPaintInvoke = unsafe fn(*mut (), PaintDevice<'_>) -> WidgetResult<()>;

#[derive(Clone, Copy)]
pub struct WidgetPaintRuntimeMeta {
    pub rust_name: &'static str,
    pub invoke: SpecPaintInvoke,
}

impl fmt::Debug for WidgetPaintRuntimeMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetPaintRuntimeMeta")
            .field("rust_name", &self.rust_name)
            .finish()
    }
}

impl PartialEq for WidgetPaintRuntimeMeta {
    fn eq(&self, other: &Self) -> bool {
        self.rust_name == other.rust_name
    }
}

impl Eq for WidgetPaintRuntimeMeta {}

pub type SpecHostEventInvoke = unsafe fn(*mut (), &[QtValue]) -> WidgetResult<()>;
pub type SpecHostOverrideInvoke =
    unsafe fn(*mut (), &mut dyn QtOpaqueHostMutDyn) -> WidgetResult<()>;

#[derive(Clone, Copy)]
pub struct WidgetHostEventRuntimeMeta {
    pub rust_name: &'static str,
    pub invoke: SpecHostEventInvoke,
}

impl fmt::Debug for WidgetHostEventRuntimeMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetHostEventRuntimeMeta")
            .field("rust_name", &self.rust_name)
            .finish()
    }
}

impl PartialEq for WidgetHostEventRuntimeMeta {
    fn eq(&self, other: &Self) -> bool {
        self.rust_name == other.rust_name
    }
}

impl Eq for WidgetHostEventRuntimeMeta {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetHostEventRuntimeSet {
    pub events: &'static [WidgetHostEventRuntimeMeta],
}

pub const NO_WIDGET_HOST_EVENT_RUNTIME: WidgetHostEventRuntimeSet =
    WidgetHostEventRuntimeSet { events: &[] };

#[derive(Clone, Copy)]
pub struct WidgetHostOverrideRuntimeMeta {
    pub rust_name: &'static str,
    pub invoke: SpecHostOverrideInvoke,
}

impl fmt::Debug for WidgetHostOverrideRuntimeMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetHostOverrideRuntimeMeta")
            .field("rust_name", &self.rust_name)
            .finish()
    }
}

impl PartialEq for WidgetHostOverrideRuntimeMeta {
    fn eq(&self, other: &Self) -> bool {
        self.rust_name == other.rust_name
    }
}

impl Eq for WidgetHostOverrideRuntimeMeta {}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct WidgetHostOverrideRuntimeSet {
    pub overrides: &'static [WidgetHostOverrideRuntimeMeta],
}

pub const NO_WIDGET_HOST_OVERRIDE_RUNTIME: WidgetHostOverrideRuntimeSet =
    WidgetHostOverrideRuntimeSet { overrides: &[] };

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HostBehaviorRuntimeDecl {
    pub host_events: &'static WidgetHostEventRuntimeSet,
    pub host_overrides: &'static WidgetHostOverrideRuntimeSet,
    pub paint: Option<WidgetPaintRuntimeMeta>,
}

pub trait QtHostBehaviorRuntimeDecl {
    fn decl() -> &'static HostBehaviorRuntimeDecl;
}

pub const NO_HOST_BEHAVIOR_RUNTIME_DECL: HostBehaviorRuntimeDecl = HostBehaviorRuntimeDecl {
    host_events: &NO_WIDGET_HOST_EVENT_RUNTIME,
    host_overrides: &NO_WIDGET_HOST_OVERRIDE_RUNTIME,
    paint: None,
};

#[doc(hidden)]
pub trait __QtHostPaintRegistration {}

#[derive(Debug, Clone, Copy)]
pub struct WidgetHostBehaviorRuntimeFragment {
    pub spec_key: SpecWidgetKey,
    pub decl: fn() -> &'static HostBehaviorRuntimeDecl,
}

#[linkme::distributed_slice]
pub static QT_WIDGET_HOST_BEHAVIOR_RUNTIME_FRAGMENTS:
    [&'static WidgetHostBehaviorRuntimeFragment];

#[derive(Default)]
struct HostBehaviorRuntimeBuilder {
    host_events: Vec<WidgetHostEventRuntimeMeta>,
    host_overrides: Vec<WidgetHostOverrideRuntimeMeta>,
    paint: Option<WidgetPaintRuntimeMeta>,
}

impl HostBehaviorRuntimeBuilder {
    fn extend(&mut self, spec_key: SpecWidgetKey, decl: &'static HostBehaviorRuntimeDecl) {
        for event in decl.host_events.events {
            if self
                .host_events
                .iter()
                .any(|existing| existing.rust_name == event.rust_name)
            {
                panic!(
                    "duplicate host event runtime {} for widget {}",
                    event.rust_name,
                    spec_key.raw()
                );
            }
            self.host_events.push(*event);
        }

        for override_meta in decl.host_overrides.overrides {
            if self
                .host_overrides
                .iter()
                .any(|existing| existing.rust_name == override_meta.rust_name)
            {
                panic!(
                    "duplicate host override runtime {} for widget {}",
                    override_meta.rust_name,
                    spec_key.raw()
                );
            }
            self.host_overrides.push(*override_meta);
        }

        if let Some(paint_meta) = decl.paint {
            if let Some(existing) = self.paint {
                panic!(
                    "duplicate host paint runtime {} and {} for widget {}",
                    existing.rust_name,
                    paint_meta.rust_name,
                    spec_key.raw()
                );
            }
            self.paint = Some(paint_meta);
        }
    }

    fn finish(self) -> &'static HostBehaviorRuntimeDecl {
        Box::leak(Box::new(HostBehaviorRuntimeDecl {
            host_events: Box::leak(Box::new(WidgetHostEventRuntimeSet {
                events: Box::leak(self.host_events.into_boxed_slice()),
            })),
            host_overrides: Box::leak(Box::new(WidgetHostOverrideRuntimeSet {
                overrides: Box::leak(self.host_overrides.into_boxed_slice()),
            })),
            paint: self.paint,
        }))
    }
}

pub fn merge_host_behavior_runtime_decls(
    context: &'static str,
    decls: &[&'static HostBehaviorRuntimeDecl],
) -> &'static HostBehaviorRuntimeDecl {
    let mut builder = HostBehaviorRuntimeBuilder::default();
    let spec_key = SpecWidgetKey::new(context);
    for decl in decls {
        builder.extend(spec_key, decl);
    }
    builder.finish()
}

pub fn resolve_widget_host_behavior(spec_key: SpecWidgetKey) -> &'static HostBehaviorRuntimeDecl {
    static DECLS: std::sync::OnceLock<BTreeMap<SpecWidgetKey, &'static HostBehaviorRuntimeDecl>> =
        std::sync::OnceLock::new();

    DECLS
        .get_or_init(|| {
            let mut builders = BTreeMap::<SpecWidgetKey, HostBehaviorRuntimeBuilder>::new();

            for fragment in QT_WIDGET_HOST_BEHAVIOR_RUNTIME_FRAGMENTS.iter().copied() {
                builders
                    .entry(fragment.spec_key)
                    .or_default()
                    .extend(fragment.spec_key, (fragment.decl)());
            }

            builders
                .into_iter()
                .map(|(key, builder)| (key, builder.finish()))
                .collect()
        })
        .get(&spec_key)
        .copied()
        .unwrap_or(&NO_HOST_BEHAVIOR_RUNTIME_DECL)
}

pub type SpecPropSetterInvoke = unsafe fn(*mut (), QtValue) -> WidgetResult<()>;
pub type SpecPropGetterInvoke = unsafe fn(*const ()) -> WidgetResult<QtValue>;

#[derive(Clone, Copy)]
pub struct WidgetPropSetterRuntimeMeta {
    pub js_name: &'static str,
    pub invoke: SpecPropSetterInvoke,
}

impl fmt::Debug for WidgetPropSetterRuntimeMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetPropSetterRuntimeMeta")
            .field("js_name", &self.js_name)
            .finish()
    }
}

#[derive(Clone, Copy)]
pub struct WidgetPropGetterRuntimeMeta {
    pub js_name: &'static str,
    pub invoke: SpecPropGetterInvoke,
}

impl fmt::Debug for WidgetPropGetterRuntimeMeta {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("WidgetPropGetterRuntimeMeta")
            .field("js_name", &self.js_name)
            .finish()
    }
}

#[derive(Debug, Clone, Copy)]
pub struct WidgetPropRuntimeDecl {
    pub setters: &'static [WidgetPropSetterRuntimeMeta],
    pub getters: &'static [WidgetPropGetterRuntimeMeta],
}

pub const NO_WIDGET_PROP_RUNTIME_DECL: WidgetPropRuntimeDecl = WidgetPropRuntimeDecl {
    setters: &[],
    getters: &[],
};

#[derive(Debug, Clone, Copy)]
pub struct WidgetPropRuntimeFragment {
    pub spec_key: SpecWidgetKey,
    pub decl: fn() -> &'static WidgetPropRuntimeDecl,
}

#[linkme::distributed_slice]
pub static QT_WIDGET_PROP_RUNTIME_FRAGMENTS: [&'static WidgetPropRuntimeFragment];

pub trait QtWidgetInstanceDyn: Send + Sync {
    fn widget_handle(&self) -> WidgetHandle;

    fn paint(&self, device: PaintDevice<'_>) -> WidgetResult<()>;

    fn invoke_opaque_hook(
        &self,
        hook_name: &str,
        host: &mut dyn QtOpaqueHostMutDyn,
    ) -> WidgetResult<()>;

    fn invoke_host_event(&self, rust_name: &str, args: &[QtValue]) -> WidgetResult<()>;

    fn invoke_host_override(
        &self,
        rust_name: &str,
        host: &mut dyn QtOpaqueHostMutDyn,
    ) -> WidgetResult<()>;

    fn apply_prop(&self, slot: u16, value: QtValue) -> WidgetResult<()>;

    fn read_prop(&self, slot: u16) -> WidgetResult<QtValue>;
}

#[derive(Default)]
struct PropRuntimeBuilder {
    setters: BTreeMap<String, WidgetPropSetterRuntimeMeta>,
    getters: BTreeMap<String, WidgetPropGetterRuntimeMeta>,
}

impl PropRuntimeBuilder {
    fn extend(&mut self, spec_key: SpecWidgetKey, decl: &'static WidgetPropRuntimeDecl) {
        for setter in decl.setters {
            if self
                .setters
                .insert(setter.js_name.to_owned(), *setter)
                .is_some()
            {
                panic!(
                    "duplicate widget prop setter runtime {} for widget {}",
                    setter.js_name,
                    spec_key.raw()
                );
            }
        }

        for getter in decl.getters {
            if self
                .getters
                .insert(getter.js_name.to_owned(), *getter)
                .is_some()
            {
                panic!(
                    "duplicate widget prop getter runtime {} for widget {}",
                    getter.js_name,
                    spec_key.raw()
                );
            }
        }
    }

    fn finish(self, spec_key: SpecWidgetKey) -> &'static WidgetPropRuntimeDecl {
        let mut setters = self.setters;
        let mut getters = self.getters;
        let merged_props = crate::schema::merged_prop_decls(spec_key, None);

        let max_setter_slot = merged_props
            .iter()
            .filter_map(|prop| prop.write_slot())
            .max()
            .map(|slot| slot as usize + 1)
            .unwrap_or(0);
        let max_getter_slot = merged_props
            .iter()
            .filter_map(|prop| prop.read_slot())
            .max()
            .map(|slot| slot as usize + 1)
            .unwrap_or(0);

        let mut setters_by_slot = vec![None; max_setter_slot];
        let mut getters_by_slot = vec![None; max_getter_slot];

        for prop in &merged_props {
            if let Some(slot) = prop.write_slot() {
                let setter = setters.remove(&prop.key).unwrap_or_else(|| {
                    panic!(
                        "missing widget prop setter runtime {} for widget {}",
                        prop.key,
                        spec_key.raw()
                    )
                });
                setters_by_slot[slot as usize] = Some(setter);
            }

            if let Some(slot) = prop.read_slot() {
                let getter = getters.remove(&prop.key).unwrap_or_else(|| {
                    panic!(
                        "missing widget prop getter runtime {} for widget {}",
                        prop.key,
                        spec_key.raw()
                    )
                });
                getters_by_slot[slot as usize] = Some(getter);
            }
        }

        if let Some((js_name, _)) = setters.into_iter().next() {
            panic!(
                "unreachable widget prop setter runtime {} for widget {}",
                js_name,
                spec_key.raw()
            );
        }

        if let Some((js_name, _)) = getters.into_iter().next() {
            panic!(
                "unreachable widget prop getter runtime {} for widget {}",
                js_name,
                spec_key.raw()
            );
        }

        Box::leak(Box::new(WidgetPropRuntimeDecl {
            setters: Box::leak(
                setters_by_slot
                    .into_iter()
                    .enumerate()
                    .map(|(slot, setter)| {
                        setter.unwrap_or_else(|| {
                            panic!(
                                "missing widget prop setter runtime for slot {} in widget {}",
                                slot,
                                spec_key.raw()
                            )
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
            getters: Box::leak(
                getters_by_slot
                    .into_iter()
                    .enumerate()
                    .map(|(slot, getter)| {
                        getter.unwrap_or_else(|| {
                            panic!(
                                "missing widget prop getter runtime for slot {} in widget {}",
                                slot,
                                spec_key.raw()
                            )
                        })
                    })
                    .collect::<Vec<_>>()
                    .into_boxed_slice(),
            ),
        }))
    }
}

pub fn resolve_widget_prop_runtime(spec_key: SpecWidgetKey) -> &'static WidgetPropRuntimeDecl {
    static DECLS: std::sync::OnceLock<BTreeMap<SpecWidgetKey, &'static WidgetPropRuntimeDecl>> =
        std::sync::OnceLock::new();

    DECLS
        .get_or_init(|| {
            let mut builders = BTreeMap::<SpecWidgetKey, PropRuntimeBuilder>::new();

            for fragment in QT_WIDGET_PROP_RUNTIME_FRAGMENTS.iter().copied() {
                builders
                    .entry(fragment.spec_key)
                    .or_default()
                    .extend(fragment.spec_key, (fragment.decl)());
            }

            builders
                .into_iter()
                .map(|(key, builder)| (key, builder.finish(key)))
                .collect()
        })
        .get(&spec_key)
        .copied()
        .unwrap_or(&NO_WIDGET_PROP_RUNTIME_DECL)
}

struct WidgetInstance<T, M> {
    handle: WidgetHandle,
    inner: Mutex<T>,
    host_behavior: &'static HostBehaviorRuntimeDecl,
    prop_runtime: &'static WidgetPropRuntimeDecl,
    marker: PhantomData<fn() -> M>,
}

impl<T, M> QtWidgetInstanceDyn for WidgetInstance<T, M>
where
    T: Send + 'static,
    M: QtOpaqueHookDispatchSurface<T> + 'static,
{
    fn widget_handle(&self) -> WidgetHandle {
        self.handle.clone()
    }

    fn paint(&self, device: PaintDevice<'_>) -> WidgetResult<()> {
        let meta = self
            .host_behavior
            .paint
            .ok_or_else(|| WidgetError::unsupported_paint_device(device.kind_name()))?;
        let mut widget = self
            .inner
            .lock()
            .map_err(|_| WidgetError::new("widget instance mutex poisoned"))?;
        unsafe { (meta.invoke)((&mut *widget as *mut T).cast::<()>(), device) }
    }

    fn invoke_opaque_hook(
        &self,
        hook_name: &str,
        host: &mut dyn QtOpaqueHostMutDyn,
    ) -> WidgetResult<()> {
        let mut widget = self
            .inner
            .lock()
            .map_err(|_| WidgetError::new("widget instance mutex poisoned"))?;
        M::__qt_invoke_opaque_hook(&mut widget, hook_name, host)
    }

    fn invoke_host_event(&self, rust_name: &str, args: &[QtValue]) -> WidgetResult<()> {
        let meta = self
            .host_behavior
            .host_events
            .events
            .iter()
            .find(|event| event.rust_name == rust_name)
            .ok_or_else(|| {
                WidgetError::new(format!("widget host event {rust_name} is not registered"))
            })?;
        let mut widget = self
            .inner
            .lock()
            .map_err(|_| WidgetError::new("widget instance mutex poisoned"))?;
        unsafe { (meta.invoke)((&mut *widget as *mut T).cast::<()>(), args) }
    }

    fn invoke_host_override(
        &self,
        rust_name: &str,
        host: &mut dyn QtOpaqueHostMutDyn,
    ) -> WidgetResult<()> {
        let meta = self
            .host_behavior
            .host_overrides
            .overrides
            .iter()
            .find(|override_meta| override_meta.rust_name == rust_name)
            .ok_or_else(|| {
                WidgetError::new(format!(
                    "widget host override {rust_name} is not registered"
                ))
            })?;
        let mut widget = self
            .inner
            .lock()
            .map_err(|_| WidgetError::new("widget instance mutex poisoned"))?;
        unsafe { (meta.invoke)((&mut *widget as *mut T).cast::<()>(), host) }
    }

    fn apply_prop(&self, slot: u16, value: QtValue) -> WidgetResult<()> {
        let mut widget = self
            .inner
            .lock()
            .map_err(|_| WidgetError::new("widget instance mutex poisoned"))?;
        let meta = self
            .prop_runtime
            .setters
            .get(slot as usize)
            .copied()
            .ok_or_else(|| {
                WidgetError::new(format!("widget prop setter slot {slot} is not registered"))
            })?;
        unsafe { (meta.invoke)((&mut *widget as *mut T).cast::<()>(), value) }
    }

    fn read_prop(&self, slot: u16) -> WidgetResult<QtValue> {
        let widget = self
            .inner
            .lock()
            .map_err(|_| WidgetError::new("widget instance mutex poisoned"))?;
        let meta = self
            .prop_runtime
            .getters
            .get(slot as usize)
            .copied()
            .ok_or_else(|| {
                WidgetError::new(format!("widget prop getter slot {slot} is not registered"))
            })?;
        unsafe { (meta.invoke)((&*widget as *const T).cast::<()>()) }
    }
}

impl<T, M> Drop for WidgetInstance<T, M> {
    fn drop(&mut self) {
        if let Ok(widget) = self.inner.lock() {
            detach_widget_handle(&*widget);
        }
    }
}

pub fn new_widget_instance<T, M>(
    handle: WidgetHandle,
    widget: T,
    host_behavior: &'static HostBehaviorRuntimeDecl,
    prop_runtime: &'static WidgetPropRuntimeDecl,
) -> Arc<dyn QtWidgetInstanceDyn>
where
    T: Send + 'static,
    M: QtOpaqueHookDispatchSurface<T> + 'static,
{
    let instance = Arc::new(WidgetInstance::<T, M> {
        handle,
        inner: Mutex::new(widget),
        host_behavior,
        prop_runtime,
        marker: PhantomData,
    });
    {
        let widget = instance
            .inner
            .lock()
            .expect("widget instance mutex poisoned during handle attachment");
        attach_widget_handle(&*widget, instance.handle.clone());
    }
    instance
}

pub type WidgetInstanceFactoryFn =
    fn(WidgetHandle, &[WidgetCreateProp]) -> WidgetResult<Arc<dyn QtWidgetInstanceDyn>>;

#[derive(Debug, Clone, Copy)]
pub struct WidgetNativeDecl {
    pub spec_key: SpecWidgetKey,
    pub create_instance: WidgetInstanceFactoryFn,
}

pub trait QtWidgetNativeDecl {
    const NATIVE_DECL: WidgetNativeDecl;
}

#[derive(Debug, Clone, Copy)]
pub struct WidgetPropDecl {
    pub spec_key: SpecWidgetKey,
    pub create_instance: Option<WidgetInstanceFactoryFn>,
    pub create_props: &'static [crate::schema::SpecCreateProp],
    pub props: &'static [crate::schema::SpecPropDecl],
}

#[derive(Debug, Clone, Copy)]
pub struct WidgetPropDeclFragment {
    pub spec_key: SpecWidgetKey,
    pub decl: fn() -> &'static [crate::schema::SpecPropDecl],
}

#[linkme::distributed_slice]
pub static QT_WIDGET_PROP_DECL_FRAGMENTS: [&'static WidgetPropDeclFragment];

pub trait QtWidgetPropDecl {
    const PROP_DECL: WidgetPropDecl;
}

#[linkme::distributed_slice]
pub static QT_WIDGET_PROP_DECLS: [&'static WidgetPropDecl];

pub fn collect_widget_prop_decls(
    spec_bindings: &[&'static crate::schema::SpecWidgetBinding],
) -> Vec<&'static WidgetPropDecl> {
    let mut decls_by_key = BTreeMap::<SpecWidgetKey, &'static WidgetPropDecl>::new();

    for decl in QT_WIDGET_PROP_DECLS.iter().copied() {
        if decls_by_key.insert(decl.spec_key, decl).is_some() {
            panic!(
                "duplicate distributed prop declaration for spec widget key {}",
                decl.spec_key.raw()
            );
        }
    }

    spec_bindings
        .iter()
        .filter_map(|spec| decls_by_key.get(&spec.spec_key).copied())
        .collect()
}

pub fn dotted_prop_path(parent: &str, segment: &str) -> String {
    if parent.is_empty() {
        segment.to_owned()
    } else {
        format!("{parent}.{segment}")
    }
}

pub fn lower_camel_prop_path(path: &str) -> String {
    let mut result = String::new();

    for (index, segment) in path.split('.').enumerate() {
        if segment.is_empty() {
            continue;
        }

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
