use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use napi::Result;
#[cfg(test)]
use qt_solid_runtime::tree::NodeTree;
use qt_solid_widget_core::runtime::{WidgetCapture, WidgetCaptureFormat};

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
};
use super::texture_widget::{
    capture_painted_widget_exact_with_children, render_texture_widget_part_into_compositor_layer,
};
use super::{
    base_upload_kind_to_compositor, capture_qt_widget_exact_with_children,
    capture_qt_widget_regions_into_capture, capture_widget_visible_rects,
    clear_window_compositor_cache, clear_window_compositor_dirty_nodes,
    compositor_target_to_renderer, effective_window_compositor_dirty_flags,
    load_window_compositor_cache, qt_rects_to_compositor, store_window_compositor_cache,
    store_window_compositor_target, take_window_compositor_dirty_nodes,
    take_window_compositor_dirty_regions, take_window_compositor_geometry_nodes,
    take_window_compositor_scene_nodes, take_window_compositor_scene_subtrees,
    upload_kind_to_compositor, widget_capture_format_to_compositor,
};

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
) -> (
    HashSet<u32>,
    Vec<WindowCompositorDirtyRegion>,
    HashSet<u32>,
    Vec<WindowCompositorDirtyRegion>,
    bool,
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
    let overlay_frame_tick = dirty_nodes.contains(&window_id);

    (
        overlay_dirty_nodes,
        overlay_dirty_region_hints,
        base_dirty_nodes,
        base_dirty_region_hints,
        overlay_frame_tick,
    )
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
    interactive_resize: bool,
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
    if has_geometry && interactive_resize {
        drop(take_window_compositor_geometry_nodes(node_id));
    }
    if has_scene {
        drop(take_window_compositor_scene_nodes(node_id));
        drop(take_window_compositor_scene_subtrees(node_id));
    }
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
        overlay_frame_tick,
    ) = split_window_overlay_dirty_state(node_id, &cached_parts, &dirty_nodes, &dirty_region_hints);
    let recapture_overlays =
        previous_cache.is_none() || has_geometry || has_scene || overlay_frame_tick;
    let parts = if recapture_overlays {
        collect_window_overlay_parts(generation, node_id, &window_bounds)?
    } else if has_pixels
        && (!overlay_dirty_nodes.is_empty() || !overlay_dirty_region_hints.is_empty())
    {
        refresh_window_parts_from_cache(
            generation,
            &cached_parts,
            &overlay_dirty_nodes,
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
    store_window_compositor_cache(node_id, current_cache.clone());
    let prepared_dirty_nodes = if recapture_overlays {
        current_cache
            .parts
            .iter()
            .map(|part| part.node_id)
            .collect::<HashSet<_>>()
    } else {
        overlay_dirty_nodes
    };
    let base_upload_kind = if previous_cache.is_none()
        || has_geometry
        || has_scene
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

pub(crate) fn present_window_with_wgpu(
    node_id: u32,
    target: QtCompositorTarget,
    stride: usize,
    scale_factor: f64,
    interactive_resize: bool,
    base_dirty_rects: Vec<QtRect>,
    bytes: &[u8],
) -> Result<bool> {
    let generation = current_app_generation()?;
    store_window_compositor_target(node_id, target);
    let dirty_flags =
        effective_window_compositor_dirty_flags(node_id, !base_dirty_rects.is_empty());
    let Some(frame) = prepare_window_compositor_frame(
        node_id,
        target.width_px,
        target.height_px,
        stride,
        scale_factor,
        dirty_flags,
        interactive_resize,
    )?
    else {
        return Ok(false);
    };
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
    let base_upload = qt_wgpu_renderer::QtCompositorBaseUpload {
        format: qt_wgpu_renderer::QtCompositorImageFormat::Bgra8UnormPremultiplied,
        width_px: target.width_px,
        height_px: target.height_px,
        stride,
        upload_kind: base_upload_kind_to_compositor(
            frame.base_upload_kind(),
            !base_dirty_rects.is_empty(),
        ),
        dirty_rects: base_dirty_rects.as_slice(),
        bytes,
    };
    let visible_rects = frame
        .parts
        .iter()
        .map(|part| {
            part.visible_rects
                .iter()
                .map(|rect| qt_wgpu_renderer::QtCompositorRect {
                    x: rect.x,
                    y: rect.y,
                    width: rect.width,
                    height: rect.height,
                })
                .collect::<Vec<_>>()
        })
        .collect::<Vec<_>>();
    let dirty_rects = frame
        .parts
        .iter()
        .map(|part| qt_rects_to_compositor(&part.dirty_rects))
        .collect::<Vec<_>>();
    let layer_uploads = frame
        .parts
        .iter()
        .zip(visible_rects.iter())
        .zip(dirty_rects.iter())
        .map(|((part, visible_rects), dirty_rects)| {
            let rendered_direct = part.source_kind
                == WindowCompositorLayerSourceKind::CachedTexture
                && render_texture_widget_part_into_compositor_layer(
                    generation,
                    render_target,
                    part.meta.node_id,
                )
                .unwrap_or(false);
            let source_kind = if rendered_direct {
                qt_wgpu_renderer::QtCompositorLayerSourceKind::CachedTexture
            } else {
                qt_wgpu_renderer::QtCompositorLayerSourceKind::CpuBytes
            };
            let bytes = if rendered_direct {
                &[][..]
            } else {
                part.capture
                    .as_deref()
                    .map(|capture| capture.bytes())
                    .or_else(|| {
                        cached_parts
                            .get(&part.meta.node_id)
                            .and_then(|entry| entry.capture())
                            .map(|capture| capture.bytes())
                    })
                    .ok_or_else(|| {
                        qt_error(format!(
                            "window compositor part {} is missing CPU fallback bytes",
                            part.meta.node_id
                        ))
                    })?
            };
            Ok(qt_wgpu_renderer::QtCompositorLayerUpload {
                node_id: part.meta.node_id,
                source_kind,
                format: widget_capture_format_to_compositor(part.meta.format_tag)?,
                x: part.meta.x,
                y: part.meta.y,
                width: part.meta.width,
                height: part.meta.height,
                width_px: part.meta.width_px,
                height_px: part.meta.height_px,
                stride: part.meta.stride,
                upload_kind: if rendered_direct {
                    qt_wgpu_renderer::QtCompositorUploadKind::None
                } else {
                    upload_kind_to_compositor(part.upload_kind)
                },
                dirty_rects: if rendered_direct {
                    &[]
                } else {
                    dirty_rects.as_slice()
                },
                visible_rects: visible_rects.as_slice(),
                bytes,
            })
        })
        .collect::<Result<Vec<_>>>()?;

    qt_wgpu_renderer::present_compositor_frame(render_target, &base_upload, &layer_uploads)
        .map_err(|error| qt_error(error.to_string()))?;
    Ok(true)
}
