use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use napi::Result;
#[cfg(test)]
use qt_solid_runtime::tree::NodeTree;
use qt_solid_widget_core::runtime::{WidgetCapture, WidgetCaptureFormat};

use crate::qt::ffi::bridge::{QtWindowCompositorDriveStatus, QtWindowCompositorPresentPlan};
use crate::{
    api::{
        QtCapturedWidgetComposingPart, QtDebugNodeBounds, QtWindowCaptureFrame,
        QtWindowCaptureGrouping,
    },
    bootstrap::widget_registry,
    qt::{self, QtRect, ffi::QtCompositorTarget},
    runtime::{
        NodeHandle, current_app_generation, debug_node_bounds, ensure_live_node, invalid_arg,
        node_by_id, node_parent_id, qt_error, subtree_node_ids,
    },
};

use super::prepare::{
    build_prepared_window_compositor_frame, collect_scene_node_dirty_regions,
    compose_window_capture_group_in_place, compose_window_capture_regions_in_place,
    dirty_region_device_bounds, merge_pixel_rects, part_device_bounds_from_dims,
};
use super::state::{
    PartVisibleRect, QtPreparedWindowCompositorFrame, WindowCaptureComposingPart,
    WindowCompositorCache, WindowCompositorDirtyFlags, WindowCompositorDirtyRegion,
    WindowCompositorLayerEntry, WindowCompositorLayerSourceKind, WindowCompositorPartUploadKind,
    WindowCompositorPendingState,
};
use super::texture_widget::{
    TextureWidgetLayerRenderResult, capture_painted_widget_exact_with_children,
    render_texture_widget_part_into_compositor_layer,
};
use super::{
    base_upload_kind_to_compositor, capture_qt_widget_exact_with_children,
    capture_qt_widget_regions_into_capture, capture_widget_visible_rects,
    clear_window_compositor_cache, clear_window_compositor_dirty_nodes,
    compositor_target_to_renderer, effective_window_compositor_dirty_flags,
    load_window_compositor_cache, qt_rects_to_compositor, snapshot_window_compositor_pending_state,
    store_window_compositor_cache, store_window_compositor_target,
    take_window_compositor_dirty_nodes, take_window_compositor_dirty_regions,
    take_window_compositor_frame_tick_nodes, take_window_compositor_geometry_nodes,
    take_window_compositor_scene_nodes, take_window_compositor_scene_subtrees,
    upload_kind_to_compositor, widget_capture_format_to_compositor,
};

fn compositor_trace_enabled() -> bool {
    std::env::var_os("QT_SOLID_WGPU_TRACE").is_some()
}

fn compositor_trace(args: std::fmt::Arguments<'_>) {
    if !compositor_trace_enabled() {
        return;
    }
    println!("[qt-pipeline] {args}");
}

fn capture_format_to_compositor(
    format: WidgetCaptureFormat,
) -> qt_wgpu_renderer::QtCompositorImageFormat {
    match format {
        WidgetCaptureFormat::Argb32Premultiplied => {
            qt_wgpu_renderer::QtCompositorImageFormat::Bgra8UnormPremultiplied
        }
        WidgetCaptureFormat::Rgba8Premultiplied => {
            qt_wgpu_renderer::QtCompositorImageFormat::Rgba8UnormPremultiplied
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum WindowCaptureGrouping {
    Segmented,
    WholeWindow,
}

impl From<QtWindowCaptureGrouping> for WindowCaptureGrouping {
    fn from(value: QtWindowCaptureGrouping) -> Self {
        match value {
            QtWindowCaptureGrouping::Segmented => Self::Segmented,
            QtWindowCaptureGrouping::WholeWindow => Self::WholeWindow,
        }
    }
}

impl From<WindowCaptureGrouping> for crate::api::QtWindowCaptureGrouping {
    fn from(value: WindowCaptureGrouping) -> Self {
        match value {
            WindowCaptureGrouping::Segmented => Self::Segmented,
            WindowCaptureGrouping::WholeWindow => Self::WholeWindow,
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct WindowCaptureGroup {
    pub(crate) parts: Vec<WindowCaptureComposingPart>,
}

#[derive(Debug, Clone)]
pub(crate) struct WindowCaptureFrame {
    pub(crate) window_id: u32,
    pub(crate) frame_seq: f64,
    pub(crate) elapsed_ms: f64,
    pub(crate) delta_ms: f64,
    pub(crate) grouping: WindowCaptureGrouping,
    pub(crate) groups: Vec<WindowCaptureGroup>,
}

impl WindowCaptureFrame {
    pub(crate) fn into_api_frame(self) -> Result<QtWindowCaptureFrame> {
        let mut parts = Vec::new();
        for group in self.groups {
            for part in group.parts {
                parts.push(part.into_debug_meta()?);
            }
        }

        Ok(QtWindowCaptureFrame {
            window_id: self.window_id,
            grouping: self.grouping.into(),
            frame_seq: self.frame_seq,
            elapsed_ms: self.elapsed_ms,
            delta_ms: self.delta_ms,
            parts,
        })
    }
}

impl WindowCaptureComposingPart {
    fn into_debug_meta(self) -> Result<QtCapturedWidgetComposingPart> {
        let stride = u32::try_from(self.capture.stride())
            .map_err(|_| qt_error("widget capture stride overflow"))?;
        let byte_length = u32::try_from(self.capture.bytes().len())
            .map_err(|_| qt_error("widget capture byte length overflow"))?;

        Ok(QtCapturedWidgetComposingPart {
            node_id: self.node_id,
            x: self.x,
            y: self.y,
            width: self.width,
            height: self.height,
            width_px: self.capture.width_px(),
            height_px: self.capture.height_px(),
            stride,
            scale_factor: self.capture.scale_factor(),
            byte_length,
        })
    }
}

pub(crate) fn window_dirty_region_to_part_local_logical_rect(
    part: &WindowCaptureComposingPart,
    region: WindowCompositorDirtyRegion,
) -> Option<PartVisibleRect> {
    let left = region.x.max(part.x);
    let top = region.y.max(part.y);
    let right = (region.x + region.width).min(part.x + part.width);
    let bottom = (region.y + region.height).min(part.y + part.height);
    (left < right && top < bottom).then_some(PartVisibleRect {
        x: left - part.x,
        y: top - part.y,
        width: right - left,
        height: bottom - top,
    })
}

fn window_compositor_layer_source_kind_for_node(
    generation: u64,
    node_id: u32,
) -> Result<WindowCompositorLayerSourceKind> {
    let node = node_by_id(generation, node_id)?;
    if node.inner().binding().host.class == "TexturePaintHostWidget" {
        return Ok(WindowCompositorLayerSourceKind::CachedTexture);
    }
    Ok(WindowCompositorLayerSourceKind::CpuCapture)
}

fn cache_entries_from_capture_parts(
    generation: u64,
    parts: Vec<WindowCaptureComposingPart>,
) -> Result<Vec<WindowCompositorLayerEntry>> {
    parts
        .into_iter()
        .map(|part| {
            let source_kind =
                window_compositor_layer_source_kind_for_node(generation, part.node_id)?;
            Ok(WindowCompositorLayerEntry::from_capture_part(
                part,
                source_kind,
            ))
        })
        .collect()
}

fn scale_logical_rect_to_compositor(
    rect: QtRect,
    scale_factor: f64,
    bounds_width_px: u32,
    bounds_height_px: u32,
) -> Option<qt_wgpu_renderer::QtCompositorRect> {
    let left = (f64::from(rect.x) * scale_factor).round() as i32;
    let top = (f64::from(rect.y) * scale_factor).round() as i32;
    let right = (f64::from(rect.x + rect.width) * scale_factor).round() as i32;
    let bottom = (f64::from(rect.y + rect.height) * scale_factor).round() as i32;

    let clipped_left = left.clamp(0, bounds_width_px as i32);
    let clipped_top = top.clamp(0, bounds_height_px as i32);
    let clipped_right = right.clamp(0, bounds_width_px as i32);
    let clipped_bottom = bottom.clamp(0, bounds_height_px as i32);
    (clipped_right > clipped_left && clipped_bottom > clipped_top).then_some(
        qt_wgpu_renderer::QtCompositorRect {
            x: clipped_left,
            y: clipped_top,
            width: clipped_right - clipped_left,
            height: clipped_bottom - clipped_top,
        },
    )
}

fn part_geometry_to_compositor(
    meta: &crate::qt::QtWindowCompositorPartMeta,
) -> Result<(i32, i32, i32, i32)> {
    let x = (f64::from(meta.x) * meta.scale_factor).round() as i32;
    let y = (f64::from(meta.y) * meta.scale_factor).round() as i32;
    let width = i32::try_from(meta.width_px)
        .map_err(|_| qt_error(format!("window compositor part {} width overflow", meta.node_id)))?;
    let height = i32::try_from(meta.height_px)
        .map_err(|_| qt_error(format!("window compositor part {} height overflow", meta.node_id)))?;
    Ok((x, y, width, height))
}

fn visible_rects_to_compositor(
    meta: &crate::qt::QtWindowCompositorPartMeta,
    visible_rects: &[QtRect],
) -> Vec<qt_wgpu_renderer::QtCompositorRect> {
    visible_rects
        .iter()
        .copied()
        .filter_map(|rect| {
            scale_logical_rect_to_compositor(rect, meta.scale_factor, meta.width_px, meta.height_px)
        })
        .collect()
}

fn cpu_capture_parts_from_layer_entries(
    parts: &[WindowCompositorLayerEntry],
) -> Result<Vec<WindowCaptureComposingPart>> {
    parts
        .iter()
        .map(|part| {
            part.to_capture_part().ok_or_else(|| {
                qt_error(format!(
                    "window compositor layer {} is missing CPU capture fallback",
                    part.node_id
                ))
            })
        })
        .collect()
}

fn capture_window_overlay_layer_exact(
    generation: u64,
    window_bounds: &QtDebugNodeBounds,
    node_id: u32,
) -> Result<Option<WindowCompositorLayerEntry>> {
    let node = node_by_id(generation, node_id)?;
    if node.inner().binding().host.class != "TexturePaintHostWidget" {
        return Ok(None);
    }
    let bounds = debug_node_bounds(node_id)?;
    if !bounds.visible || bounds.width <= 0 || bounds.height <= 0 {
        return Ok(None);
    }

    let visible_rects = capture_widget_visible_rects(node_id)?;
    if visible_rects.is_empty() {
        return Ok(None);
    }
    let layout =
        qt::qt_capture_widget_layout(node_id).map_err(|error| qt_error(error.what().to_owned()))?;

    Ok(Some(WindowCompositorLayerEntry::cached_texture(
        node_id,
        bounds.screen_x - window_bounds.screen_x,
        bounds.screen_y - window_bounds.screen_y,
        bounds.width,
        bounds.height,
        visible_rects,
        layout.width_px,
        layout.height_px,
        layout.scale_factor,
    )))
}

fn collect_window_overlay_parts(
    generation: u64,
    window_id: u32,
    window_bounds: &QtDebugNodeBounds,
) -> Result<Vec<WindowCompositorLayerEntry>> {
    let subtree_ids = subtree_node_ids(generation, window_id)?;
    let mut parts = Vec::new();
    for node_id in subtree_ids {
        if let Some(part) = capture_window_overlay_layer_exact(generation, window_bounds, node_id)?
        {
            parts.push(part);
        }
    }

    Ok(parts)
}

fn capture_window_part_exact(
    generation: u64,
    window_bounds: &QtDebugNodeBounds,
    node_id: u32,
    allow_cached_vello: bool,
) -> Result<Option<WindowCaptureComposingPart>> {
    let node = node_by_id(generation, node_id)?;
    let bounds = debug_node_bounds(node_id)?;
    if !bounds.visible || bounds.width <= 0 || bounds.height <= 0 {
        return Ok(None);
    }

    let visible_rects = capture_widget_visible_rects(node_id)?;
    if visible_rects.is_empty() {
        return Ok(None);
    }

    let capture = if allow_cached_vello {
        capture_painted_widget_exact_with_children(&node, false)?
    } else {
        capture_qt_widget_exact_with_children(&node, false)?
    };
    Ok(Some(WindowCaptureComposingPart {
        node_id,
        x: bounds.screen_x - window_bounds.screen_x,
        y: bounds.screen_y - window_bounds.screen_y,
        width: bounds.width,
        height: bounds.height,
        visible_rects,
        capture: Arc::new(capture),
    }))
}

pub(crate) fn split_window_overlay_dirty_state(
    window_id: u32,
    cached_parts: &[WindowCompositorLayerEntry],
    dirty_nodes: &HashSet<u32>,
    dirty_region_hints: &[WindowCompositorDirtyRegion],
    frame_tick_nodes: &HashSet<u32>,
) -> (
    HashSet<u32>,
    Vec<WindowCompositorDirtyRegion>,
    HashSet<u32>,
    Vec<WindowCompositorDirtyRegion>,
    HashSet<u32>,
) {
    let cached_node_ids: HashSet<u32> = cached_parts.iter().map(|part| part.node_id).collect();
    let overlay_dirty_nodes = dirty_nodes
        .iter()
        .copied()
        .filter(|node_id| cached_node_ids.contains(node_id))
        .collect::<HashSet<_>>();
    let overlay_dirty_region_hints = dirty_region_hints
        .iter()
        .copied()
        .filter(|region| cached_node_ids.contains(&region.node_id))
        .collect::<Vec<_>>();
    let base_dirty_nodes = dirty_nodes
        .iter()
        .copied()
        .filter(|node_id| *node_id != window_id && !cached_node_ids.contains(node_id))
        .collect::<HashSet<_>>();
    let base_dirty_region_hints = dirty_region_hints
        .iter()
        .copied()
        .filter(|region| !cached_node_ids.contains(&region.node_id))
        .collect::<Vec<_>>();
    let overlay_frame_tick_nodes = frame_tick_nodes
        .iter()
        .copied()
        .filter(|node_id| cached_node_ids.contains(node_id))
        .collect::<HashSet<_>>();

    (
        overlay_dirty_nodes,
        overlay_dirty_region_hints,
        base_dirty_nodes,
        base_dirty_region_hints,
        overlay_frame_tick_nodes,
    )
}

fn layer_entry_metadata_matches(
    previous: &WindowCompositorLayerEntry,
    current: &WindowCompositorLayerEntry,
) -> bool {
    previous.node_id == current.node_id
        && previous.x == current.x
        && previous.y == current.y
        && previous.width == current.width
        && previous.height == current.height
        && previous.visible_rects == current.visible_rects
        && previous.format_tag == current.format_tag
        && previous.width_px == current.width_px
        && previous.height_px == current.height_px
        && previous.stride == current.stride
        && (previous.scale_factor - current.scale_factor).abs() <= 0.001
        && previous.source_kind() == current.source_kind()
}

fn diff_overlay_layout(
    previous_cache: Option<&WindowCompositorCache>,
    current_cache: &WindowCompositorCache,
) -> (bool, HashSet<u32>) {
    let Some(previous_cache) = previous_cache else {
        return (
            !current_cache.parts.is_empty(),
            current_cache
                .parts
                .iter()
                .map(|part| part.node_id)
                .collect(),
        );
    };

    let previous_parts = previous_cache
        .parts
        .iter()
        .map(|part| (part.node_id, part))
        .collect::<HashMap<_, _>>();
    let current_parts = current_cache
        .parts
        .iter()
        .map(|part| (part.node_id, part))
        .collect::<HashMap<_, _>>();
    let mut changed_nodes = HashSet::new();
    let mut layout_changed = previous_cache.parts.len() != current_cache.parts.len();

    for current in &current_cache.parts {
        match previous_parts.get(&current.node_id).copied() {
            Some(previous) if layer_entry_metadata_matches(previous, current) => {}
            _ => {
                changed_nodes.insert(current.node_id);
                layout_changed = true;
            }
        }
    }

    for previous in &previous_cache.parts {
        if !current_parts.contains_key(&previous.node_id) {
            layout_changed = true;
        }
    }

    (layout_changed, changed_nodes)
}

fn plan_window_compositor_present_for_state(
    window_id: u32,
    cache: Option<&WindowCompositorCache>,
    pending_state: &WindowCompositorPendingState,
    has_base_dirty_rects: bool,
) -> QtWindowCompositorPresentPlan {
    let Some(cache) = cache else {
        return QtWindowCompositorPresentPlan {
            must_present: true,
            needs_base_upload: true,
            cached_width_px: 0,
            cached_height_px: 0,
            cached_stride: 0,
        };
    };

    let overlay_node_ids = cache
        .parts
        .iter()
        .map(|part| part.node_id)
        .collect::<HashSet<_>>();
    let has_base_pixel_dirty = pending_state
        .dirty_nodes
        .iter()
        .any(|node_id| *node_id != window_id && !overlay_node_ids.contains(node_id))
        || pending_state
            .dirty_regions
            .iter()
            .any(|region| !overlay_node_ids.contains(&region.node_id));
    let has_base_layout_dirty = pending_state
        .geometry_nodes
        .iter()
        .any(|node_id| !overlay_node_ids.contains(node_id))
        || pending_state
            .scene_nodes
            .iter()
            .any(|node_id| !overlay_node_ids.contains(node_id))
        || pending_state
            .scene_subtrees
            .iter()
            .any(|node_id| *node_id != window_id && !overlay_node_ids.contains(node_id));
    let has_overlay_pixel_dirty = pending_state
        .dirty_nodes
        .iter()
        .any(|node_id| overlay_node_ids.contains(node_id))
        || pending_state
            .dirty_regions
            .iter()
            .any(|region| overlay_node_ids.contains(&region.node_id))
        || pending_state
            .frame_tick_nodes
            .iter()
            .any(|node_id| overlay_node_ids.contains(node_id));
    let has_overlay_layout_dirty = pending_state
        .geometry_nodes
        .iter()
        .any(|node_id| overlay_node_ids.contains(node_id))
        || pending_state
            .scene_nodes
            .iter()
            .any(|node_id| overlay_node_ids.contains(node_id))
        || pending_state
            .scene_subtrees
            .iter()
            .any(|node_id| overlay_node_ids.contains(node_id) || *node_id == window_id);
    let needs_base_upload = has_base_pixel_dirty || has_base_layout_dirty;
    let must_present = has_base_dirty_rects
        || needs_base_upload
        || has_overlay_pixel_dirty
        || has_overlay_layout_dirty;

    QtWindowCompositorPresentPlan {
        must_present,
        needs_base_upload,
        cached_width_px: cache.width_px,
        cached_height_px: cache.height_px,
        cached_stride: cache.stride,
    }
}

pub(crate) fn coalesce_scene_subtree_roots(
    generation: u64,
    roots: &HashSet<u32>,
) -> Result<HashSet<u32>> {
    if roots.is_empty() {
        return Ok(HashSet::new());
    }

    let mut minimal = HashSet::new();
    'candidate: for root in roots {
        let mut current = node_parent_id(generation, *root)?;
        while let Some(parent_id) = current {
            if roots.contains(&parent_id) {
                continue 'candidate;
            }
            current = node_parent_id(generation, parent_id)?;
        }
        minimal.insert(*root);
    }

    Ok(minimal)
}

#[cfg(test)]
pub(crate) fn coalesce_scene_subtree_roots_in_tree(
    tree: &NodeTree,
    roots: &HashSet<u32>,
) -> HashSet<u32> {
    if roots.is_empty() {
        return HashSet::new();
    }

    let mut minimal = HashSet::new();
    'candidate: for root in roots {
        let mut current = tree.get_parent(*root);
        while let Some(parent_id) = current {
            if roots.contains(&parent_id) {
                continue 'candidate;
            }
            current = tree.get_parent(parent_id);
        }
        minimal.insert(*root);
    }

    minimal
}

fn minimize_scene_subtree_roots(generation: u64, roots: &HashSet<u32>) -> Result<HashSet<u32>> {
    coalesce_scene_subtree_roots(generation, roots)
}

pub(crate) fn prepare_window_compositor_frame(
    node_id: u32,
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    dirty_flags: u8,
) -> Result<Option<Box<QtPreparedWindowCompositorFrame>>> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before preparing a window compositor",
        ));
    }

    let generation = current_app_generation()?;
    let window = node_by_id(generation, node_id)?;
    let class = ensure_live_node(&window)?;
    let binding = widget_registry().binding_for_node_class(class);
    if binding.kind_name != "window" {
        return Err(invalid_arg(format!(
            "node {node_id} is not a window widget"
        )));
    }

    let layout =
        qt::qt_capture_widget_layout(node_id).map_err(|error| qt_error(error.what().to_owned()))?;
    if layout.width_px != width_px
        || layout.height_px != height_px
        || layout.stride != stride
        || (layout.scale_factor - scale_factor).abs() > 0.001
    {
        clear_window_compositor_cache(node_id);
        return Ok(None);
    }

    let previous_cache = load_window_compositor_cache(node_id);
    let dirty_flags = WindowCompositorDirtyFlags::from_bits(dirty_flags);
    let has_geometry = dirty_flags.contains(WindowCompositorDirtyFlags::GEOMETRY);
    let has_scene = dirty_flags.contains(WindowCompositorDirtyFlags::SCENE);
    let has_pixels = dirty_flags.contains(WindowCompositorDirtyFlags::PIXELS);
    let geometry_dirty_nodes = if has_geometry {
        take_window_compositor_geometry_nodes(node_id)
    } else {
        HashSet::new()
    };
    let scene_dirty_nodes = if has_scene {
        take_window_compositor_scene_nodes(node_id)
    } else {
        HashSet::new()
    };
    let scene_dirty_subtrees = if has_scene {
        take_window_compositor_scene_subtrees(node_id)
    } else {
        HashSet::new()
    };
    let frame_tick_nodes = if has_pixels {
        take_window_compositor_frame_tick_nodes(node_id)
    } else {
        HashSet::new()
    };
    let dirty_nodes = if has_pixels {
        take_window_compositor_dirty_nodes(node_id)
    } else {
        HashSet::new()
    };
    let dirty_region_hints = if has_pixels {
        take_window_compositor_dirty_regions(node_id)
    } else {
        Vec::new()
    };
    if has_geometry || has_scene {
        clear_window_compositor_dirty_nodes(node_id);
    }
    let window_bounds = debug_node_bounds(node_id)?;
    let cached_parts = previous_cache
        .as_ref()
        .map(|cache| cache.parts.clone())
        .unwrap_or_default();
    let (
        overlay_dirty_nodes,
        overlay_dirty_region_hints,
        base_dirty_nodes,
        base_dirty_region_hints,
        overlay_frame_tick_nodes,
    ) = split_window_overlay_dirty_state(
        node_id,
        &cached_parts,
        &dirty_nodes,
        &dirty_region_hints,
        &frame_tick_nodes,
    );
    let recapture_overlay_metadata = previous_cache.is_none() || has_geometry || has_scene;
    let overlay_refresh_nodes = overlay_dirty_nodes
        .union(&overlay_frame_tick_nodes)
        .copied()
        .collect::<HashSet<_>>();
    let parts = if recapture_overlay_metadata {
        collect_window_overlay_parts(generation, node_id, &window_bounds)?
    } else if has_pixels
        && (!overlay_refresh_nodes.is_empty() || !overlay_dirty_region_hints.is_empty())
    {
        refresh_window_parts_from_cache(
            generation,
            &cached_parts,
            &overlay_refresh_nodes,
            &overlay_dirty_region_hints,
            true,
        )?
    } else {
        cached_parts
    };
    let current_cache = WindowCompositorCache {
        generation,
        width_px: layout.width_px,
        height_px: layout.height_px,
        stride: layout.stride,
        scale_factor: layout.scale_factor,
        parts,
    };
    let (overlay_layout_changed, changed_overlay_nodes) =
        diff_overlay_layout(previous_cache.as_ref(), &current_cache);
    let overlay_node_ids = current_cache
        .parts
        .iter()
        .map(|part| part.node_id)
        .collect::<HashSet<_>>();
    let base_layout_dirty = geometry_dirty_nodes
        .iter()
        .any(|node_id| !overlay_node_ids.contains(node_id))
        || scene_dirty_nodes
            .iter()
            .any(|node_id| !overlay_node_ids.contains(node_id))
        || scene_dirty_subtrees.iter().any(|dirty_node_id| {
            *dirty_node_id != node_id && !overlay_node_ids.contains(dirty_node_id)
        });
    store_window_compositor_cache(node_id, current_cache.clone());
    let prepared_dirty_nodes = if previous_cache.is_none() {
        current_cache
            .parts
            .iter()
            .map(|part| part.node_id)
            .collect::<HashSet<_>>()
    } else {
        overlay_refresh_nodes
            .union(&changed_overlay_nodes)
            .copied()
            .collect::<HashSet<_>>()
    };
    let base_upload_kind = if previous_cache.is_none()
        || base_layout_dirty
        || !base_dirty_nodes.is_empty()
        || !base_dirty_region_hints.is_empty()
    {
        WindowCompositorPartUploadKind::Full
    } else {
        WindowCompositorPartUploadKind::None
    };
    Ok(Some(build_prepared_window_compositor_frame(
        &current_cache,
        previous_cache.as_ref(),
        dirty_flags,
        &prepared_dirty_nodes,
        &overlay_dirty_region_hints,
        base_upload_kind,
        overlay_layout_changed,
    )?))
}

fn refresh_window_parts_from_cache(
    generation: u64,
    cached_parts: &[WindowCompositorLayerEntry],
    dirty_nodes: &HashSet<u32>,
    dirty_region_hints: &[WindowCompositorDirtyRegion],
    reuse_cached_geometry: bool,
) -> Result<Vec<WindowCompositorLayerEntry>> {
    if dirty_nodes.is_empty() {
        return Ok(cached_parts.to_vec());
    }

    let cached_node_ids: HashSet<u32> = cached_parts.iter().map(|part| part.node_id).collect();
    if !dirty_nodes.is_subset(&cached_node_ids) {
        return Err(qt_error(
            "window compositor dirty nodes no longer match cached parts",
        ));
    }

    let mut parts = Vec::with_capacity(cached_parts.len());
    for cached in cached_parts {
        if dirty_nodes.contains(&cached.node_id) {
            let node = node_by_id(generation, cached.node_id)?;
            let (x, y, width, height, visible_rects) = if reuse_cached_geometry {
                (
                    cached.x,
                    cached.y,
                    cached.width,
                    cached.height,
                    cached.visible_rects.clone(),
                )
            } else {
                let bounds = debug_node_bounds(cached.node_id)?;
                if !bounds.visible || bounds.width <= 0 || bounds.height <= 0 {
                    continue;
                }
                let visible_rects = capture_widget_visible_rects(cached.node_id)?;
                if visible_rects.is_empty() {
                    continue;
                }
                (
                    cached.x,
                    cached.y,
                    bounds.width,
                    bounds.height,
                    visible_rects,
                )
            };
            if cached.source_kind() == WindowCompositorLayerSourceKind::CachedTexture {
                let layout = qt::qt_capture_widget_layout(cached.node_id)
                    .map_err(|error| qt_error(error.what().to_owned()))?;
                parts.push(WindowCompositorLayerEntry::cached_texture(
                    cached.node_id,
                    x,
                    y,
                    width,
                    height,
                    visible_rects,
                    layout.width_px,
                    layout.height_px,
                    layout.scale_factor,
                ));
            } else {
                let local_dirty_regions = dirty_region_hints
                    .iter()
                    .filter(|region| region.node_id == cached.node_id)
                    .filter_map(|region| {
                        cached.to_capture_part().and_then(|part| {
                            window_dirty_region_to_part_local_logical_rect(&part, *region)
                        })
                    })
                    .collect::<Vec<_>>();
                let capture = if local_dirty_regions.is_empty() {
                    capture_qt_widget_exact_with_children(&node, false)?
                } else if cached.capture().is_some_and(|capture| {
                    capture.format() == WidgetCaptureFormat::Argb32Premultiplied
                }) {
                    let Some(existing_capture) = cached.capture() else {
                        return Err(qt_error(format!(
                            "window compositor layer {} is missing CPU capture fallback",
                            cached.node_id
                        )));
                    };
                    let mut capture = existing_capture.as_ref().clone();
                    capture_qt_widget_regions_into_capture(
                        &node,
                        false,
                        &mut capture,
                        &local_dirty_regions,
                    )?;
                    capture
                } else {
                    capture_qt_widget_exact_with_children(&node, false)?
                };
                parts.push(WindowCompositorLayerEntry::from_capture_part(
                    WindowCaptureComposingPart {
                        node_id: cached.node_id,
                        x,
                        y,
                        width,
                        height,
                        visible_rects,
                        capture: Arc::new(capture),
                    },
                    WindowCompositorLayerSourceKind::CpuCapture,
                ));
            }
        } else {
            parts.push(cached.clone());
        }
    }

    Ok(parts)
}

pub(crate) fn collect_window_capture_parts(
    generation: u64,
    window_id: u32,
    window_bounds: &QtDebugNodeBounds,
    allow_cached_vello: bool,
) -> Result<Vec<WindowCaptureComposingPart>> {
    let subtree_ids = subtree_node_ids(generation, window_id)?;
    let mut parts = Vec::new();
    for node_id in subtree_ids {
        if let Some(part) =
            capture_window_part_exact(generation, window_bounds, node_id, allow_cached_vello)?
        {
            parts.push(part);
        }
    }

    Ok(parts)
}

pub(crate) fn capture_window_widget_exact(window: &impl NodeHandle) -> Result<WidgetCapture> {
    ensure_live_node(window)?;
    capture_qt_widget_exact_with_children(window, true)
}

pub(crate) fn group_window_capture_parts(
    grouping: WindowCaptureGrouping,
    parts: Vec<WindowCaptureComposingPart>,
) -> Vec<Vec<WindowCaptureComposingPart>> {
    match grouping {
        WindowCaptureGrouping::Segmented => parts.into_iter().map(|part| vec![part]).collect(),
        WindowCaptureGrouping::WholeWindow => {
            if parts.is_empty() {
                Vec::new()
            } else {
                vec![parts]
            }
        }
    }
}

pub(crate) fn capture_window_frame_exact(
    window_id: u32,
    grouping: WindowCaptureGrouping,
) -> Result<WindowCaptureFrame> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before capturing a window frame",
        ));
    }

    let generation = current_app_generation()?;
    let window = node_by_id(generation, window_id)?;
    let class = ensure_live_node(&window)?;
    let binding = widget_registry().binding_for_node_class(class);
    if binding.kind_name != "window" {
        return Err(invalid_arg(format!(
            "node {window_id} is not a window widget"
        )));
    }

    let window_bounds = debug_node_bounds(window_id)?;
    let frame_seq = super::frame_clock::read_frame_f64_prop(&window, "seq")?;
    let elapsed_ms = super::frame_clock::read_frame_f64_prop(&window, "elapsedMs")?;
    let delta_ms = super::frame_clock::read_frame_f64_prop(&window, "deltaMs")?;
    qt::qt_capture_widget_layout(window_id).map_err(|error| qt_error(error.what().to_owned()))?;
    let parts = collect_window_capture_parts(generation, window_id, &window_bounds, true)?;
    let groups = group_window_capture_parts(grouping, parts)
        .into_iter()
        .map(|parts| WindowCaptureGroup { parts })
        .collect();

    Ok(WindowCaptureFrame {
        window_id,
        frame_seq,
        elapsed_ms,
        delta_ms,
        grouping,
        groups,
    })
}

fn rebuild_window_compositor_frame(
    generation: u64,
    window_id: u32,
    layout: &qt::QtWidgetCaptureLayout,
    allow_cached_vello: bool,
    bytes: &mut [u8],
) -> Result<()> {
    let window_bounds = debug_node_bounds(window_id)?;
    let parts =
        collect_window_capture_parts(generation, window_id, &window_bounds, allow_cached_vello)?;
    compose_window_capture_group_in_place(
        bytes,
        layout.width_px,
        layout.height_px,
        layout.stride,
        layout.scale_factor,
        &parts,
    )?;
    let cache = WindowCompositorCache {
        generation,
        width_px: layout.width_px,
        height_px: layout.height_px,
        stride: layout.stride,
        scale_factor: layout.scale_factor,
        parts: cache_entries_from_capture_parts(generation, parts)?,
    };
    store_window_compositor_cache(window_id, cache);
    Ok(())
}

pub(crate) fn resize_reuse_cache_compatible(
    cache: &WindowCompositorCache,
    generation: u64,
    layout: &qt::QtWidgetCaptureLayout,
) -> bool {
    cache.generation == generation && (cache.scale_factor - layout.scale_factor).abs() <= 0.001
}

fn reuse_window_compositor_resize_frame(
    generation: u64,
    window_id: u32,
    layout: &qt::QtWidgetCaptureLayout,
    geometry_dirty_nodes: &HashSet<u32>,
    bytes: &mut [u8],
) -> Result<Option<()>> {
    let Some(cache) = load_window_compositor_cache(window_id) else {
        return Ok(None);
    };
    if !resize_reuse_cache_compatible(&cache, generation, layout) {
        return Ok(None);
    }

    let window_bounds = debug_node_bounds(window_id)?;
    let cached_capture_parts = cpu_capture_parts_from_layer_entries(&cache.parts)?;
    let cached_parts: HashMap<u32, &WindowCaptureComposingPart> = cached_capture_parts
        .iter()
        .map(|part| (part.node_id, part))
        .collect();
    let mut parts = Vec::with_capacity(cache.parts.len().max(geometry_dirty_nodes.len()));

    for cached in &cached_capture_parts {
        if !geometry_dirty_nodes.contains(&cached.node_id) {
            parts.push(cached.clone());
            continue;
        }

        let node = node_by_id(generation, cached.node_id)?;
        let bounds = debug_node_bounds(cached.node_id)?;
        if !bounds.visible || bounds.width <= 0 || bounds.height <= 0 {
            continue;
        }

        let visible_rects = capture_widget_visible_rects(cached.node_id)?;
        if visible_rects.is_empty() {
            continue;
        }

        let capture = if cached.width == bounds.width && cached.height == bounds.height {
            cached.capture.clone()
        } else {
            Arc::new(capture_painted_widget_exact_with_children(&node, false)?)
        };

        parts.push(WindowCaptureComposingPart {
            node_id: cached.node_id,
            x: bounds.screen_x - window_bounds.screen_x,
            y: bounds.screen_y - window_bounds.screen_y,
            width: bounds.width,
            height: bounds.height,
            visible_rects,
            capture,
        });
    }

    for node_id in geometry_dirty_nodes {
        if cached_parts.contains_key(node_id) {
            continue;
        }

        let node = node_by_id(generation, *node_id)?;
        let bounds = debug_node_bounds(*node_id)?;
        if !bounds.visible || bounds.width <= 0 || bounds.height <= 0 {
            continue;
        }

        let visible_rects = capture_widget_visible_rects(*node_id)?;
        if visible_rects.is_empty() {
            continue;
        }

        parts.push(WindowCaptureComposingPart {
            node_id: *node_id,
            x: bounds.screen_x - window_bounds.screen_x,
            y: bounds.screen_y - window_bounds.screen_y,
            width: bounds.width,
            height: bounds.height,
            visible_rects,
            capture: Arc::new(capture_painted_widget_exact_with_children(&node, false)?),
        });
    }

    compose_window_capture_group_in_place(
        bytes,
        layout.width_px,
        layout.height_px,
        layout.stride,
        layout.scale_factor,
        &parts,
    )?;
    let refreshed_cache = WindowCompositorCache {
        generation,
        width_px: layout.width_px,
        height_px: layout.height_px,
        stride: layout.stride,
        scale_factor: layout.scale_factor,
        parts: cache_entries_from_capture_parts(generation, parts)?,
    };
    store_window_compositor_cache(window_id, refreshed_cache);
    Ok(Some(()))
}

fn reuse_window_compositor_scene_frame(
    generation: u64,
    window_id: u32,
    layout: &qt::QtWidgetCaptureLayout,
    dirty_scene_nodes: &HashSet<u32>,
    dirty_scene_subtrees: &HashSet<u32>,
    bytes: &mut [u8],
) -> Result<Option<()>> {
    let Some(cache) = load_window_compositor_cache(window_id) else {
        return Ok(None);
    };
    if cache.generation != generation
        || cache.width_px != layout.width_px
        || cache.height_px != layout.height_px
        || cache.stride != layout.stride
        || (cache.scale_factor - layout.scale_factor).abs() > 0.001
    {
        return Ok(None);
    }
    if (dirty_scene_nodes.is_empty() && dirty_scene_subtrees.is_empty())
        || dirty_scene_nodes.contains(&window_id)
        || dirty_scene_subtrees.contains(&window_id)
    {
        return Ok(None);
    }

    let window_bounds = debug_node_bounds(window_id)?;
    let window_subtree_ids = subtree_node_ids(generation, window_id)?;
    let cached_capture_parts = cpu_capture_parts_from_layer_entries(&cache.parts)?;
    let cached_parts: HashMap<u32, WindowCaptureComposingPart> = cached_capture_parts
        .iter()
        .cloned()
        .map(|part| (part.node_id, part))
        .collect();

    if dirty_scene_subtrees.is_empty() {
        let mut parts = Vec::new();
        let mut new_dirty_parts = HashMap::new();
        for node_id in window_subtree_ids {
            if dirty_scene_nodes.contains(&node_id) {
                if let Some(part) =
                    capture_window_part_exact(generation, &window_bounds, node_id, false)?
                {
                    new_dirty_parts.insert(node_id, part.clone());
                    parts.push(part);
                }
            } else if let Some(cached) = cached_parts.get(&node_id) {
                parts.push(cached.clone());
            }
        }

        let dirty_regions = collect_scene_node_dirty_regions(
            layout.width_px,
            layout.height_px,
            layout.scale_factor,
            dirty_scene_nodes,
            &cached_parts,
            &new_dirty_parts,
        )?;
        if !dirty_regions.is_empty() {
            compose_window_capture_regions_in_place(
                bytes,
                layout.width_px,
                layout.height_px,
                layout.stride,
                layout.scale_factor,
                &parts,
                &dirty_regions,
            )?;
        }
        let refreshed_cache = WindowCompositorCache {
            generation,
            width_px: layout.width_px,
            height_px: layout.height_px,
            stride: layout.stride,
            scale_factor: layout.scale_factor,
            parts: cache_entries_from_capture_parts(generation, parts)?,
        };
        store_window_compositor_cache(window_id, refreshed_cache);
        return Ok(Some(()));
    }

    let minimal_subtree_roots = minimize_scene_subtree_roots(generation, dirty_scene_subtrees)?;
    let mut affected_subtree_nodes = HashSet::new();
    for node_id in &minimal_subtree_roots {
        for subtree_id in subtree_node_ids(generation, *node_id)? {
            affected_subtree_nodes.insert(subtree_id);
        }
    }
    let mut parts = Vec::new();
    for node_id in window_subtree_ids {
        if affected_subtree_nodes.contains(&node_id) || dirty_scene_nodes.contains(&node_id) {
            let allow_cached_vello = !dirty_scene_nodes.contains(&node_id);
            if let Some(part) =
                capture_window_part_exact(generation, &window_bounds, node_id, allow_cached_vello)?
            {
                parts.push(part);
            }
        } else if let Some(cached) = cached_parts.get(&node_id) {
            parts.push(cached.clone());
        } else {
            return Ok(None);
        }
    }

    compose_window_capture_group_in_place(
        bytes,
        layout.width_px,
        layout.height_px,
        layout.stride,
        layout.scale_factor,
        &parts,
    )?;
    let refreshed_cache = WindowCompositorCache {
        generation,
        width_px: layout.width_px,
        height_px: layout.height_px,
        stride: layout.stride,
        scale_factor: layout.scale_factor,
        parts: cache_entries_from_capture_parts(generation, parts)?,
    };
    store_window_compositor_cache(window_id, refreshed_cache);
    Ok(Some(()))
}

fn reuse_window_compositor_frame(
    generation: u64,
    window_id: u32,
    layout: &qt::QtWidgetCaptureLayout,
    refresh_pixels: bool,
    dirty_nodes: &HashSet<u32>,
    dirty_region_hints: &[WindowCompositorDirtyRegion],
    bytes: &mut [u8],
) -> Result<Option<()>> {
    let Some(cache) = load_window_compositor_cache(window_id) else {
        return Ok(None);
    };
    if cache.generation != generation
        || cache.width_px != layout.width_px
        || cache.height_px != layout.height_px
        || cache.stride != layout.stride
        || (cache.scale_factor - layout.scale_factor).abs() > 0.001
    {
        return Ok(None);
    }

    if !refresh_pixels {
        return Ok(Some(()));
    }

    let parts = match refresh_window_parts_from_cache(
        generation,
        &cache.parts,
        dirty_nodes,
        dirty_region_hints,
        true,
    ) {
        Ok(parts) => parts,
        Err(_) => return Ok(None),
    };
    let mut dirty_regions = Vec::new();
    let mut nodes_with_region_hints = HashSet::new();
    for region_hint in dirty_region_hints {
        nodes_with_region_hints.insert(region_hint.node_id);
        if let Some(region) = dirty_region_device_bounds(
            layout.width_px,
            layout.height_px,
            layout.scale_factor,
            *region_hint,
        )? {
            dirty_regions.push(region);
        }
    }
    for part in &parts {
        if dirty_nodes.contains(&part.node_id) && !nodes_with_region_hints.contains(&part.node_id) {
            let Some(capture_part) = part.to_capture_part() else {
                return Ok(None);
            };
            if let Some(region) = part_device_bounds_from_dims(
                layout.width_px,
                layout.height_px,
                layout.scale_factor,
                &capture_part,
            )? {
                dirty_regions.push(region);
            }
        }
    }
    let dirty_regions = merge_pixel_rects(dirty_regions);
    if !dirty_regions.is_empty() {
        let capture_parts = cpu_capture_parts_from_layer_entries(&parts)?;
        compose_window_capture_regions_in_place(
            bytes,
            layout.width_px,
            layout.height_px,
            layout.stride,
            layout.scale_factor,
            &capture_parts,
            &dirty_regions,
        )?;
    }
    let refreshed_cache = WindowCompositorCache {
        generation,
        width_px: layout.width_px,
        height_px: layout.height_px,
        stride: layout.stride,
        scale_factor: layout.scale_factor,
        parts,
    };
    store_window_compositor_cache(window_id, refreshed_cache);
    Ok(Some(()))
}

pub(crate) fn paint_window_compositor(
    node_id: u32,
    width_px: u32,
    height_px: u32,
    stride: usize,
    scale_factor: f64,
    dirty_flags: u8,
    interactive_resize: bool,
    bytes: &mut [u8],
) -> Result<bool> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before painting a window compositor",
        ));
    }

    let generation = current_app_generation()?;
    let window = node_by_id(generation, node_id)?;
    let class = ensure_live_node(&window)?;
    let binding = widget_registry().binding_for_node_class(class);
    if binding.kind_name != "window" {
        return Err(invalid_arg(format!(
            "node {node_id} is not a window widget"
        )));
    }

    let layout =
        qt::qt_capture_widget_layout(node_id).map_err(|error| qt_error(error.what().to_owned()))?;
    if layout.width_px != width_px
        || layout.height_px != height_px
        || layout.stride != stride
        || (layout.scale_factor - scale_factor).abs() > 0.001
    {
        clear_window_compositor_cache(node_id);
        return Ok(false);
    }
    let required_len = layout
        .stride
        .checked_mul(height_px as usize)
        .ok_or_else(|| qt_error("window compositor target buffer size overflow"))?;
    if bytes.len() < required_len {
        return Err(qt_error(
            "window compositor target buffer is smaller than required",
        ));
    }
    let dirty_flags = WindowCompositorDirtyFlags::from_bits(dirty_flags);
    let has_geometry = dirty_flags.contains(WindowCompositorDirtyFlags::GEOMETRY);
    let has_scene = dirty_flags.contains(WindowCompositorDirtyFlags::SCENE);
    let has_pixels = dirty_flags.contains(WindowCompositorDirtyFlags::PIXELS);
    let geometry_dirty_nodes = if has_geometry && interactive_resize {
        take_window_compositor_geometry_nodes(node_id)
    } else {
        HashSet::new()
    };
    let (scene_dirty_nodes, scene_dirty_subtrees) = if has_scene {
        (
            take_window_compositor_scene_nodes(node_id),
            take_window_compositor_scene_subtrees(node_id),
        )
    } else {
        (HashSet::new(), HashSet::new())
    };
    let (dirty_nodes, dirty_region_hints) = if has_pixels {
        (
            take_window_compositor_dirty_nodes(node_id),
            take_window_compositor_dirty_regions(node_id),
        )
    } else {
        (HashSet::new(), Vec::new())
    };

    if has_geometry {
        clear_window_compositor_dirty_nodes(node_id);
        if interactive_resize
            && reuse_window_compositor_resize_frame(
                generation,
                node_id,
                &layout,
                &geometry_dirty_nodes,
                bytes,
            )?
            .is_none()
        {
            rebuild_window_compositor_frame(generation, node_id, &layout, !has_pixels, bytes)?;
        } else if !interactive_resize {
            rebuild_window_compositor_frame(generation, node_id, &layout, !has_pixels, bytes)?;
        }
    } else {
        if has_scene
            && reuse_window_compositor_scene_frame(
                generation,
                node_id,
                &layout,
                &scene_dirty_nodes,
                &scene_dirty_subtrees,
                bytes,
            )?
            .is_none()
        {
            clear_window_compositor_dirty_nodes(node_id);
            rebuild_window_compositor_frame(generation, node_id, &layout, false, bytes)?;
            return Ok(true);
        }

        if has_pixels {
            if reuse_window_compositor_frame(
                generation,
                node_id,
                &layout,
                true,
                &dirty_nodes,
                &dirty_region_hints,
                bytes,
            )?
            .is_none()
            {
                clear_window_compositor_dirty_nodes(node_id);
                rebuild_window_compositor_frame(generation, node_id, &layout, false, bytes)?;
            }
        } else if !has_scene
            && reuse_window_compositor_frame(
                generation,
                node_id,
                &layout,
                false,
                &HashSet::new(),
                &[],
                bytes,
            )?
            .is_none()
        {
            rebuild_window_compositor_frame(generation, node_id, &layout, true, bytes)?;
        }
    }

    Ok(true)
}

fn present_window_with_wgpu_impl(
    node_id: u32,
    target: QtCompositorTarget,
    stride: usize,
    scale_factor: f64,
    needs_base_upload: bool,
    base_dirty_rects: Vec<QtRect>,
    bytes: &[u8],
    async_present: bool,
) -> Result<bool> {
    let generation = current_app_generation()?;
    store_window_compositor_target(node_id, target);
    let has_base_dirty_rects = !base_dirty_rects.is_empty();
    let dirty_flags = effective_window_compositor_dirty_flags(node_id, needs_base_upload);
    let Some(frame) = prepare_window_compositor_frame(
        node_id,
        target.width_px,
        target.height_px,
        stride,
        scale_factor,
        dirty_flags,
    )?
    else {
        return Ok(false);
    };
    if !prepared_frame_requires_present(&frame, has_base_dirty_rects) {
        let render_target =
            compositor_target_to_renderer(target).map_err(|error| qt_error(error.to_string()))?;
        qt_wgpu_renderer::record_compositor_present_decision(render_target, false);
        return Ok(true);
    }
    let current_cache = load_window_compositor_cache(node_id);
    let cached_parts = current_cache
        .as_ref()
        .map(|cache| {
            cache
                .parts
                .iter()
                .map(|part| (part.node_id, part))
                .collect::<HashMap<_, _>>()
        })
        .unwrap_or_default();

    let render_target =
        compositor_target_to_renderer(target).map_err(|error| qt_error(error.to_string()))?;
    let base_dirty_rects = qt_rects_to_compositor(&base_dirty_rects);
    let mac_can_reuse_presented_base = matches!(
        qt_wgpu_renderer::current_backend_kind(),
        qt_wgpu_renderer::CompositorBackendKind::Macos
    ) && qt_wgpu_renderer::compositor_frame_is_initialized(render_target)
        && bytes.is_empty()
        && !has_base_dirty_rects;
    let frame_base_upload_kind = if mac_can_reuse_presented_base {
        WindowCompositorPartUploadKind::None
    } else {
        frame.base_upload_kind()
    };
    let effective_base_upload =
        needs_base_upload || frame_base_upload_kind != WindowCompositorPartUploadKind::None;
    if effective_base_upload && bytes.is_empty() {
        return Err(qt_error(
            "window compositor requires backingstore bytes for base upload",
        ));
    }
    let base_upload = qt_wgpu_renderer::QtCompositorBaseUpload {
        format: qt_wgpu_renderer::QtCompositorImageFormat::Bgra8UnormPremultiplied,
        width_px: target.width_px,
        height_px: target.height_px,
        stride,
        upload_kind: base_upload_kind_to_compositor(
            frame_base_upload_kind,
            effective_base_upload && has_base_dirty_rects,
        ),
        dirty_rects: base_dirty_rects.as_slice(),
        bytes,
    };
    let visible_rects = frame
        .parts
        .iter()
        .map(|part| visible_rects_to_compositor(&part.meta, &part.visible_rects))
        .collect::<Vec<_>>();
    let dirty_rects = frame
        .parts
        .iter()
        .map(|part| qt_rects_to_compositor(&part.dirty_rects))
        .collect::<Vec<_>>();
    let mut use_cached_texture_by_index = Vec::with_capacity(frame.parts.len());
    let mut fallback_captures = HashMap::new();
    for (index, part) in frame.parts.iter().enumerate() {
        let direct_render_result = if part.source_kind
            == WindowCompositorLayerSourceKind::CachedTexture
            && part.needs_layer_redraw
        {
            render_texture_widget_part_into_compositor_layer(
                generation,
                render_target,
                part.meta.node_id,
            )
            .ok()
        } else if part.source_kind == WindowCompositorLayerSourceKind::CachedTexture {
            Some(TextureWidgetLayerRenderResult {
                rendered: false,
                next_frame_requested: false,
                local_dirty_rects_px: Vec::new(),
            })
        } else {
            None
        };
        let use_cached_texture = if part.source_kind == WindowCompositorLayerSourceKind::CachedTexture {
            !part.needs_layer_redraw || direct_render_result.is_some()
        } else {
            false
        };
        if !use_cached_texture
            && part.capture.is_none()
            && cached_parts
                .get(&part.meta.node_id)
                .and_then(|entry| entry.capture())
                .is_none()
            && part.source_kind == WindowCompositorLayerSourceKind::CachedTexture
        {
            let node = node_by_id(generation, part.meta.node_id)?;
            let capture = capture_painted_widget_exact_with_children(&node, false)?;
            fallback_captures.insert(part.meta.node_id, capture);
        }
        if use_cached_texture_by_index.len() != index {
            return Err(qt_error("window compositor layer planning index drifted"));
        }
        use_cached_texture_by_index.push(use_cached_texture);
    }

    let mut layer_uploads = Vec::with_capacity(frame.parts.len());
    for (((part, visible_rects), dirty_rects), use_cached_texture) in frame
        .parts
        .iter()
        .zip(visible_rects.iter())
        .zip(dirty_rects.iter())
        .zip(use_cached_texture_by_index.iter().copied())
    {
        let source_kind = if use_cached_texture {
            qt_wgpu_renderer::QtCompositorLayerSourceKind::CachedTexture
        } else {
            qt_wgpu_renderer::QtCompositorLayerSourceKind::CpuBytes
        };
        let selected_capture = if use_cached_texture {
            None
        } else if let Some(capture) = part.capture.as_deref() {
            Some(capture)
        } else if let Some(capture) = cached_parts
            .get(&part.meta.node_id)
            .and_then(|entry| entry.capture())
            .map(Arc::as_ref)
        {
            Some(capture)
        } else if part.source_kind == WindowCompositorLayerSourceKind::CachedTexture {
            Some(
                fallback_captures
                    .get(&part.meta.node_id)
                    .ok_or_else(|| {
                        qt_error(format!(
                            "window compositor part {} planned cached-texture fallback but capture is missing",
                            part.meta.node_id
                        ))
                    })?,
            )
        } else {
            return Err(qt_error(format!(
                "window compositor part {} is missing CPU fallback bytes",
                part.meta.node_id
            )));
        };
        let bytes = selected_capture.map_or(&[][..], WidgetCapture::bytes);
        let format = if let Some(capture) = selected_capture {
            capture_format_to_compositor(capture.format())
        } else {
            widget_capture_format_to_compositor(part.meta.format_tag)?
        };
        let width_px = selected_capture
            .map(WidgetCapture::width_px)
            .unwrap_or(part.meta.width_px);
        let height_px = selected_capture
            .map(WidgetCapture::height_px)
            .unwrap_or(part.meta.height_px);
        let stride = selected_capture
            .map(WidgetCapture::stride)
            .unwrap_or(part.meta.stride);
        compositor_trace(format_args!(
            "drive-layer node={} source={:?} upload={:?} capture={} bytes={} format={:?} size={}x{} visible_rects={}",
            part.meta.node_id,
            source_kind,
            if use_cached_texture {
                qt_wgpu_renderer::QtCompositorUploadKind::None
            } else {
                upload_kind_to_compositor(part.upload_kind)
            },
            selected_capture.is_some(),
            bytes.len(),
            format,
            width_px,
            height_px,
            visible_rects.len()
        ));
        let (x, y, width, height) = part_geometry_to_compositor(&part.meta)?;
        layer_uploads.push(qt_wgpu_renderer::QtCompositorLayerUpload {
            node_id: part.meta.node_id,
            source_kind,
            format,
            x,
            y,
            width,
            height,
            width_px,
            height_px,
            stride,
            upload_kind: if use_cached_texture {
                qt_wgpu_renderer::QtCompositorUploadKind::None
            } else {
                upload_kind_to_compositor(part.upload_kind)
            },
            dirty_rects: if use_cached_texture {
                &[]
            } else {
                dirty_rects.as_slice()
            },
            visible_rects: visible_rects.as_slice(),
            bytes,
        });
    }

    if async_present {
        qt_wgpu_renderer::present_compositor_frame_async(
            node_id,
            render_target,
            &base_upload,
            &layer_uploads,
        )
        .map_err(|error| qt_error(error.to_string()))?;
    } else {
        qt_wgpu_renderer::load_or_create_compositor(render_target)
            .and_then(|compositor| {
                compositor.present_frame(render_target, &base_upload, &layer_uploads, Some(node_id))
            })
            .map_err(|error| qt_error(error.to_string()))?;
    }
    Ok(true)
}

pub(crate) fn present_window_with_wgpu(
    node_id: u32,
    target: QtCompositorTarget,
    stride: usize,
    scale_factor: f64,
    needs_base_upload: bool,
    base_dirty_rects: Vec<QtRect>,
    bytes: &[u8],
) -> Result<bool> {
    if matches!(
        qt_wgpu_renderer::current_backend_kind(),
        qt_wgpu_renderer::CompositorBackendKind::Macos
    ) {
        store_window_compositor_target(node_id, target);
        let render_target =
            compositor_target_to_renderer(target).map_err(|error| qt_error(error.to_string()))?;
        let base_dirty_rects = qt_rects_to_compositor(&base_dirty_rects);
        let base_upload = qt_wgpu_renderer::QtCompositorBaseUpload {
            format: qt_wgpu_renderer::QtCompositorImageFormat::Bgra8UnormPremultiplied,
            width_px: target.width_px,
            height_px: target.height_px,
            stride,
            upload_kind: if needs_base_upload {
                base_upload_kind_to_compositor(WindowCompositorPartUploadKind::Full, true)
            } else {
                qt_wgpu_renderer::QtCompositorUploadKind::None
            },
            dirty_rects: base_dirty_rects.as_slice(),
            bytes,
        };
        let layer_uploads = Vec::new();
        qt_wgpu_renderer::load_or_create_compositor(render_target)
            .and_then(|compositor| {
                compositor.ingest_frame(node_id, render_target, &base_upload, &layer_uploads)
            })
            .map_err(|error| qt_error(error.to_string()))?;
        return Ok(true);
    }

    present_window_with_wgpu_impl(
        node_id,
        target,
        stride,
        scale_factor,
        needs_base_upload,
        base_dirty_rects,
        bytes,
        false,
    )
}

pub(crate) fn plan_present_window_with_wgpu(
    node_id: u32,
    base_dirty_rects: Vec<QtRect>,
) -> Result<QtWindowCompositorPresentPlan> {
    let cache = load_window_compositor_cache(node_id);
    let pending_state = snapshot_window_compositor_pending_state(node_id);
    Ok(plan_window_compositor_present_for_state(
        node_id,
        cache.as_ref(),
        &pending_state,
        !base_dirty_rects.is_empty(),
    ))
}

pub(crate) fn drive_window_compositor_frame(
    node_id: u32,
    target: QtCompositorTarget,
) -> Result<QtWindowCompositorDriveStatus> {
    let cache = load_window_compositor_cache(node_id);
    let pending_state = snapshot_window_compositor_pending_state(node_id);
    let plan =
        plan_window_compositor_present_for_state(node_id, cache.as_ref(), &pending_state, false);
    compositor_trace(format_args!(
        "drive node={} target={}x{} must_present={} needs_base_upload={} cached={}x{} stride={}",
        node_id,
        target.width_px,
        target.height_px,
        plan.must_present,
        plan.needs_base_upload,
        plan.cached_width_px,
        plan.cached_height_px,
        plan.cached_stride
    ));
    if !plan.must_present {
        return Ok(QtWindowCompositorDriveStatus::Idle);
    }
    let render_target =
        compositor_target_to_renderer(target).map_err(|error| qt_error(error.to_string()))?;
    let mac_can_reuse_presented_base = matches!(
        qt_wgpu_renderer::current_backend_kind(),
        qt_wgpu_renderer::CompositorBackendKind::Macos
    ) && qt_wgpu_renderer::compositor_frame_is_initialized(render_target);
    if plan.needs_base_upload && !mac_can_reuse_presented_base {
        return Ok(QtWindowCompositorDriveStatus::NeedsQtRepaint);
    }
    if qt_wgpu_renderer::compositor_frame_is_busy(render_target) {
        return Ok(QtWindowCompositorDriveStatus::Busy);
    }
    let async_present = !matches!(
        qt_wgpu_renderer::current_backend_kind(),
        qt_wgpu_renderer::CompositorBackendKind::Macos
    );
    let (stride, scale_factor, needs_base_upload, bytes) = if plan.needs_base_upload
        && mac_can_reuse_presented_base
    {
        let layout =
            qt::qt_capture_widget_layout(node_id).map_err(|error| qt_error(error.what().to_owned()))?;
        (layout.stride, layout.scale_factor, false, &[][..])
    } else {
        (plan.cached_stride, target.scale_factor, false, &[][..])
    };

    let presented = present_window_with_wgpu_impl(
        node_id,
        target,
        stride,
        scale_factor,
        needs_base_upload,
        Vec::new(),
        bytes,
        async_present,
    )?;
    compositor_trace(format_args!(
        "drive-present node={} target={}x{} presented={}",
        node_id,
        target.width_px,
        target.height_px,
        presented
    ));
    if presented {
        Ok(QtWindowCompositorDriveStatus::Presented)
    } else {
        Ok(QtWindowCompositorDriveStatus::NeedsQtRepaint)
    }
}

fn prepared_frame_requires_present(
    frame: &QtPreparedWindowCompositorFrame,
    has_base_dirty_rects: bool,
) -> bool {
    if has_base_dirty_rects || frame.base_upload_kind() != WindowCompositorPartUploadKind::None {
        return true;
    }
    if frame.overlay_layout_changed {
        return true;
    }

    frame.parts.iter().any(|part| {
        part.needs_layer_redraw || part.upload_kind != WindowCompositorPartUploadKind::None
    })
}

#[cfg(test)]
mod tests {
    use super::{
        part_geometry_to_compositor, prepared_frame_requires_present,
        scale_logical_rect_to_compositor, visible_rects_to_compositor,
    };
    use crate::{
        qt::{QtRect, QtWindowCompositorPartMeta},
        window_compositor::state::{
            QtPreparedWindowCompositorFrame, QtPreparedWindowCompositorPart,
            WindowCompositorLayerSourceKind, WindowCompositorPartUploadKind,
        },
    };

    fn part_meta() -> QtWindowCompositorPartMeta {
        QtWindowCompositorPartMeta {
            node_id: 7,
            format_tag: 2,
            x: 12,
            y: 18,
            width: 128,
            height: 128,
            width_px: 256,
            height_px: 256,
            stride: 0,
            scale_factor: 2.0,
        }
    }

    fn prepared_part() -> QtPreparedWindowCompositorPart {
        QtPreparedWindowCompositorPart {
            meta: part_meta(),
            visible_rects: vec![],
            upload_kind: WindowCompositorPartUploadKind::None,
            dirty_rects: vec![],
            source_kind: WindowCompositorLayerSourceKind::CachedTexture,
            needs_layer_redraw: false,
            capture: None,
        }
    }

    #[test]
    fn compositor_geometry_uses_device_pixels() {
        let meta = part_meta();
        assert_eq!(
            part_geometry_to_compositor(&meta).expect("geometry conversion should succeed"),
            (24, 36, 256, 256)
        );
    }

    #[test]
    fn compositor_visible_rects_are_scaled_and_clamped() {
        let meta = part_meta();
        let rects = visible_rects_to_compositor(
            &meta,
            &[QtRect {
                x: 0,
                y: 0,
                width: 128,
                height: 128,
            }],
        );
        assert_eq!(rects.len(), 1);
        assert_eq!(rects[0].x, 0);
        assert_eq!(rects[0].y, 0);
        assert_eq!(rects[0].width, 256);
        assert_eq!(rects[0].height, 256);
    }

    #[test]
    fn compositor_scaled_rect_drops_empty_after_clamp() {
        let rect = scale_logical_rect_to_compositor(
            QtRect {
                x: 300,
                y: 300,
                width: 10,
                height: 10,
            },
            2.0,
            256,
            256,
        );
        assert!(rect.is_none());
    }

    #[test]
    fn prepared_frame_without_dirty_skips_present() {
        let frame = QtPreparedWindowCompositorFrame {
            base_upload_kind: WindowCompositorPartUploadKind::None,
            overlay_layout_changed: false,
            parts: vec![prepared_part()],
        };

        assert!(!prepared_frame_requires_present(&frame, false));
    }

    #[test]
    fn prepared_frame_with_base_dirty_forces_present() {
        let frame = QtPreparedWindowCompositorFrame {
            base_upload_kind: WindowCompositorPartUploadKind::None,
            overlay_layout_changed: false,
            parts: vec![prepared_part()],
        };

        assert!(prepared_frame_requires_present(&frame, true));
    }

    #[test]
    fn prepared_frame_with_layer_redraw_forces_present() {
        let mut part = prepared_part();
        part.needs_layer_redraw = true;
        let frame = QtPreparedWindowCompositorFrame {
            base_upload_kind: WindowCompositorPartUploadKind::None,
            overlay_layout_changed: false,
            parts: vec![part],
        };

        assert!(prepared_frame_requires_present(&frame, false));
    }

    #[test]
    fn prepared_frame_with_upload_forces_present() {
        let mut part = prepared_part();
        part.upload_kind = WindowCompositorPartUploadKind::Full;
        let frame = QtPreparedWindowCompositorFrame {
            base_upload_kind: WindowCompositorPartUploadKind::None,
            overlay_layout_changed: false,
            parts: vec![part],
        };

        assert!(prepared_frame_requires_present(&frame, false));
    }

    #[test]
    fn prepared_frame_with_overlay_layout_change_forces_present() {
        let frame = QtPreparedWindowCompositorFrame {
            base_upload_kind: WindowCompositorPartUploadKind::None,
            overlay_layout_changed: true,
            parts: vec![prepared_part()],
        };

        assert!(prepared_frame_requires_present(&frame, false));
    }
}
