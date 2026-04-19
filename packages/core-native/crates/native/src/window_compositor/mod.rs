pub(crate) mod bridge;
pub(crate) mod frame_clock;
pub(crate) mod pipeline;
pub(crate) mod prepare;
pub(crate) mod state;
pub(crate) mod texture_widget;

use std::collections::HashSet;

use self::state::{
    PartVisibleRect, WindowCompositorCache, WindowCompositorDirtyFlags,
    WindowCompositorDirtyRegion, WindowCompositorPartUploadKind,
};
use napi::Result;
use qt_solid_widget_core::runtime::{WidgetCapture, WidgetCaptureFormat};

use crate::{
    qt::{self, QtRect, ffi::QtCompositorTarget},
    runtime::{NodeHandle, qt_error},
};

pub(crate) use bridge::{
    qt_mark_window_compositor_geometry_dirty, qt_mark_window_compositor_pixels_dirty,
    qt_mark_window_compositor_pixels_dirty_region, qt_mark_window_compositor_scene_dirty,
    qt_paint_window_compositor, qt_prepare_window_compositor_frame, qt_present_window_with_wgpu,
    qt_window_compositor_frame_base_upload_kind, qt_window_compositor_frame_part_bytes,
    qt_window_compositor_frame_part_count, qt_window_compositor_frame_part_dirty_rects,
    qt_window_compositor_frame_part_meta, qt_window_compositor_frame_part_upload_kind,
    qt_window_compositor_frame_part_visible_rects, qt_window_frame_tick,
    qt_window_take_next_frame_request,
};
pub(crate) use frame_clock::{
    read_frame_f64_prop, window_ancestor_id_for_node, write_frame_bool_prop,
};
pub(crate) use pipeline::{
    WindowCaptureGrouping, capture_window_frame_exact, capture_window_widget_exact,
};
pub(crate) use state::{CompositorState, QtPreparedWindowCompositorFrame};
pub(crate) use texture_widget::capture_painted_widget_exact_with_children;

fn load_window_compositor_cache(window_id: u32) -> Option<WindowCompositorCache> {
    crate::runtime::with_compositor_state(|state| state.cache(window_id).cloned())
}

fn store_window_compositor_cache(window_id: u32, cache: WindowCompositorCache) {
    crate::runtime::with_compositor_state_mut(|state| state.set_cache(window_id, cache));
}

fn clear_window_compositor_cache(window_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.clear_cache(window_id));
}

fn store_window_compositor_target(window_id: u32, target: QtCompositorTarget) {
    crate::runtime::with_compositor_state_mut(|state| state.set_target(window_id, target));
}

fn load_window_compositor_target(window_id: u32) -> Option<QtCompositorTarget> {
    crate::runtime::with_compositor_state(|state| state.target(window_id))
}

fn mark_window_compositor_scene_node(window_id: u32, node_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.mark_scene_node(window_id, node_id));
}

fn mark_window_compositor_geometry_node(window_id: u32, node_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.mark_geometry_node(window_id, node_id));
}

pub(crate) fn mark_window_compositor_scene_subtree(window_id: u32, node_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.mark_scene_subtree(window_id, node_id));
}

fn mark_window_compositor_dirty_node(window_id: u32, node_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.mark_dirty_node(window_id, node_id));
}

fn mark_window_compositor_dirty_region(
    window_id: u32,
    node_id: u32,
    x: i32,
    y: i32,
    width: i32,
    height: i32,
) {
    if width <= 0 || height <= 0 {
        return;
    }

    crate::runtime::with_compositor_state_mut(|state| {
        state.mark_dirty_region(
            window_id,
            WindowCompositorDirtyRegion {
                node_id,
                x,
                y,
                width,
                height,
            },
        )
    });
}

fn take_window_compositor_dirty_nodes(window_id: u32) -> HashSet<u32> {
    crate::runtime::with_compositor_state_mut(|state| state.take_dirty_nodes(window_id))
}

fn take_window_compositor_scene_nodes(window_id: u32) -> HashSet<u32> {
    crate::runtime::with_compositor_state_mut(|state| state.take_scene_nodes(window_id))
}

fn take_window_compositor_geometry_nodes(window_id: u32) -> HashSet<u32> {
    crate::runtime::with_compositor_state_mut(|state| state.take_geometry_nodes(window_id))
}

fn take_window_compositor_scene_subtrees(window_id: u32) -> HashSet<u32> {
    crate::runtime::with_compositor_state_mut(|state| state.take_scene_subtrees(window_id))
}

fn take_window_compositor_dirty_regions(window_id: u32) -> Vec<WindowCompositorDirtyRegion> {
    crate::runtime::with_compositor_state_mut(|state| state.take_dirty_regions(window_id))
}

fn clear_window_compositor_dirty_nodes(window_id: u32) {
    crate::runtime::with_compositor_state_mut(|state| state.clear_dirty_nodes(window_id));
}

fn effective_window_compositor_dirty_flags(window_id: u32, has_base_pixels_dirty: bool) -> u8 {
    let mut flags =
        crate::runtime::with_compositor_state(|state| state.pending_dirty_flags(window_id));
    if has_base_pixels_dirty {
        flags = flags | WindowCompositorDirtyFlags::PIXELS;
    }
    flags.bits()
}

fn upload_kind_tag(kind: WindowCompositorPartUploadKind) -> u8 {
    match kind {
        WindowCompositorPartUploadKind::None => 0,
        WindowCompositorPartUploadKind::Full => 1,
        WindowCompositorPartUploadKind::SubRects => 2,
    }
}

fn compositor_surface_kind_to_renderer(
    kind: crate::qt::ffi::QtCompositorSurfaceKind,
) -> Result<u8> {
    let surface_kind = match kind {
        crate::qt::ffi::QtCompositorSurfaceKind::AppKitNsView => {
            qt_wgpu_renderer::QT_COMPOSITOR_SURFACE_APPKIT_NS_VIEW
        }
        crate::qt::ffi::QtCompositorSurfaceKind::Win32Hwnd => {
            qt_wgpu_renderer::QT_COMPOSITOR_SURFACE_WIN32_HWND
        }
        crate::qt::ffi::QtCompositorSurfaceKind::XcbWindow => {
            qt_wgpu_renderer::QT_COMPOSITOR_SURFACE_XCB_WINDOW
        }
        crate::qt::ffi::QtCompositorSurfaceKind::WaylandSurface => {
            qt_wgpu_renderer::QT_COMPOSITOR_SURFACE_WAYLAND_SURFACE
        }
        _ => return Err(qt_error("unsupported qt compositor surface kind tag")),
    };
    Ok(surface_kind)
}

fn compositor_target_to_renderer(
    target: QtCompositorTarget,
) -> Result<qt_wgpu_renderer::QtCompositorTarget> {
    Ok(qt_wgpu_renderer::QtCompositorTarget {
        surface_kind: compositor_surface_kind_to_renderer(target.surface_kind)?,
        primary_handle: target.primary_handle,
        secondary_handle: target.secondary_handle,
        width_px: target.width_px,
        height_px: target.height_px,
        scale_factor: target.scale_factor,
    })
}

fn upload_kind_to_compositor(
    kind: WindowCompositorPartUploadKind,
) -> qt_wgpu_renderer::QtCompositorUploadKind {
    match kind {
        WindowCompositorPartUploadKind::None => qt_wgpu_renderer::QtCompositorUploadKind::None,
        WindowCompositorPartUploadKind::Full => qt_wgpu_renderer::QtCompositorUploadKind::Full,
        WindowCompositorPartUploadKind::SubRects => {
            qt_wgpu_renderer::QtCompositorUploadKind::SubRects
        }
    }
}

fn base_upload_kind_to_compositor(
    kind: WindowCompositorPartUploadKind,
    has_base_dirty_rects: bool,
) -> qt_wgpu_renderer::QtCompositorUploadKind {
    match kind {
        WindowCompositorPartUploadKind::None if has_base_dirty_rects => {
            qt_wgpu_renderer::QtCompositorUploadKind::SubRects
        }
        _ => upload_kind_to_compositor(kind),
    }
}

fn qt_rects_to_compositor(rects: &[QtRect]) -> Vec<qt_wgpu_renderer::QtCompositorRect> {
    rects
        .iter()
        .map(|rect| qt_wgpu_renderer::QtCompositorRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        })
        .collect()
}

fn widget_capture_format_to_compositor(
    format_tag: u8,
) -> Result<qt_wgpu_renderer::QtCompositorImageFormat> {
    match format_tag {
        1 => Ok(qt_wgpu_renderer::QtCompositorImageFormat::Bgra8UnormPremultiplied),
        2 => Ok(qt_wgpu_renderer::QtCompositorImageFormat::Rgba8UnormPremultiplied),
        other => Err(qt_error(format!(
            "unsupported compositor image format tag {other}",
        ))),
    }
}

fn widget_capture_format_from_qt(tag: u8) -> Result<WidgetCaptureFormat> {
    match tag {
        1 => Ok(WidgetCaptureFormat::Argb32Premultiplied),
        2 => Ok(WidgetCaptureFormat::Rgba8Premultiplied),
        _ => Err(qt_error(format!(
            "unsupported Qt widget capture format tag {tag}",
        ))),
    }
}

fn capture_qt_widget_exact_with_children(
    node: &impl NodeHandle,
    include_children: bool,
) -> Result<WidgetCapture> {
    crate::runtime::ensure_live_node(node)?;

    let layout = qt::qt_capture_widget_layout(node.inner().id)
        .map_err(|error| qt_error(error.what().to_owned()))?;
    let format = widget_capture_format_from_qt(layout.format_tag)?;
    let mut capture = WidgetCapture::new_zeroed(
        format,
        layout.width_px,
        layout.height_px,
        layout.stride,
        layout.scale_factor,
    )
    .map_err(|error| qt_error(error.message().to_owned()))?;

    qt::qt_capture_widget_into(
        node.inner().id,
        layout.width_px,
        layout.height_px,
        layout.stride,
        include_children,
        capture.bytes_mut(),
    )
    .map_err(|error| qt_error(error.what().to_owned()))?;

    Ok(capture)
}

fn capture_qt_widget_regions_into_capture(
    node: &impl NodeHandle,
    include_children: bool,
    capture: &mut WidgetCapture,
    regions: &[PartVisibleRect],
) -> Result<()> {
    crate::runtime::ensure_live_node(node)?;
    if regions.is_empty() {
        return Ok(());
    }
    if capture.format() != WidgetCaptureFormat::Argb32Premultiplied {
        return Err(qt_error(
            "partial Qt widget capture requires argb32 premultiplied backing",
        ));
    }

    let layout = qt::qt_capture_widget_layout(node.inner().id)
        .map_err(|error| qt_error(error.what().to_owned()))?;
    let format = widget_capture_format_from_qt(layout.format_tag)?;
    if format != capture.format()
        || layout.width_px != capture.width_px()
        || layout.height_px != capture.height_px()
        || layout.stride != capture.stride()
        || (layout.scale_factor - capture.scale_factor()).abs() > 0.001
    {
        return Err(qt_error(
            "qt widget capture layout changed during partial refresh",
        ));
    }

    for region in regions {
        if region.width <= 0 || region.height <= 0 {
            continue;
        }
        qt::qt_capture_widget_region_into(
            node.inner().id,
            layout.width_px,
            layout.height_px,
            layout.stride,
            include_children,
            QtRect {
                x: region.x,
                y: region.y,
                width: region.width,
                height: region.height,
            },
            capture.bytes_mut(),
        )
        .map_err(|error| qt_error(error.what().to_owned()))?;
    }

    Ok(())
}

fn capture_widget_visible_rects(node_id: u32) -> Result<Vec<PartVisibleRect>> {
    let rects = qt::qt_capture_widget_visible_rects(node_id)
        .map_err(|error| qt_error(error.what().to_owned()))?;
    Ok(rects
        .into_iter()
        .filter(|rect| rect.width > 0 && rect.height > 0)
        .map(|rect| PartVisibleRect {
            x: rect.x,
            y: rect.y,
            width: rect.width,
            height: rect.height,
        })
        .collect())
}
