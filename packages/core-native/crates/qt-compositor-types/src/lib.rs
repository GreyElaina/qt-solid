use std::{fmt, sync::Arc};

pub type Result<T> = std::result::Result<T, QtCompositorError>;

#[derive(Debug, Clone)]
pub struct QtCompositorError {
    message: Arc<str>,
}

impl QtCompositorError {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: Arc::from(message.into()),
        }
    }
}

impl fmt::Display for QtCompositorError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.message)
    }
}

impl std::error::Error for QtCompositorError {}

pub const QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW: u8 = 1;
pub const QT_COMPOSITOR_SURFACE_WIN32_HWND: u8 = 2;
pub const QT_COMPOSITOR_SURFACE_XCB_WINDOW: u8 = 3;
pub const QT_COMPOSITOR_SURFACE_WAYLAND_SURFACE: u8 = 4;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct QtCompositorSurfaceKey {
    pub surface_kind: u8,
    pub primary_handle: u64,
    pub secondary_handle: u64,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct QtCompositorTarget {
    pub surface_kind: u8,
    pub primary_handle: u64,
    pub secondary_handle: u64,
    pub width_px: u32,
    pub height_px: u32,
    pub scale_factor: f64,
}

impl QtCompositorTarget {
    pub fn surface_key(self) -> QtCompositorSurfaceKey {
        QtCompositorSurfaceKey {
            surface_kind: self.surface_kind,
            primary_handle: self.primary_handle,
            secondary_handle: self.secondary_handle,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtCompositorImageFormat {
    Bgra8UnormPremultiplied,
    Rgba8UnormPremultiplied,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtCompositorUploadKind {
    None,
    Full,
    SubRects,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QtCompositorLayerSourceKind {
    CpuBytes,
    CachedTexture,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct QtCompositorRect {
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
}

#[derive(Debug, Clone, Copy)]
pub struct QtCompositorBaseUpload<'a> {
    pub format: QtCompositorImageFormat,
    pub width_px: u32,
    pub height_px: u32,
    pub stride: usize,
    pub upload_kind: QtCompositorUploadKind,
    pub dirty_rects: &'a [QtCompositorRect],
    pub bytes: &'a [u8],
}

#[derive(Debug, Clone, Copy)]
pub struct QtCompositorLayerUpload<'a> {
    pub node_id: u32,
    pub source_kind: QtCompositorLayerSourceKind,
    pub format: QtCompositorImageFormat,
    pub x: i32,
    pub y: i32,
    pub width: i32,
    pub height: i32,
    pub width_px: u32,
    pub height_px: u32,
    pub stride: usize,
    pub upload_kind: QtCompositorUploadKind,
    pub dirty_rects: &'a [QtCompositorRect],
    pub visible_rects: &'a [QtCompositorRect],
    pub bytes: &'a [u8],
}
