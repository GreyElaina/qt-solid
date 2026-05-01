use std::sync::Arc;

use napi::Result;
use crate::runtime::capture::WidgetCapture;

use crate::qt::ffi::bridge::{QtMotionMouseTarget, QtWindowCompositorDriveStatus};
use crate::{
    api::{
        QtCapturedWidgetComposingPart, QtDebugNodeBounds, QtWindowCaptureFrame,
        QtWindowCaptureGrouping,
    },
    qt::{
        self,
        ffi::QtCompositorTarget,
    },
    runtime::{
        current_app_generation, debug_node_bounds, ensure_live_node, invalid_arg,
        node_by_id, qt_error, subtree_node_ids,
        NodeHandle,
    },
};

use super::state::WindowCaptureComposingPart;
use super::texture_widget::capture_painted_widget_exact_with_children;
use super::{
    capture_qt_widget_exact_with_children, capture_widget_visible_rects, clear_window_compositor_dirty_nodes,
    compositor_target_to_renderer, load_window_compositor_target,
    snapshot_window_compositor_pending_state, store_window_compositor_target,
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
        capture: Arc::new(capture),
    }))
}

#[cfg(test)]
pub(crate) fn coalesce_scene_subtree_roots_in_tree(
    tree: &crate::runtime::tree::NodeTree,
    roots: &std::collections::HashSet<u32>,
) -> std::collections::HashSet<u32> {
    use std::collections::HashSet;
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
    ensure_live_node(&window)?;
    if !window.inner().is_window() {
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


pub(crate) fn drive_window_compositor_frame(
    node_id: u32,
    target: QtCompositorTarget,
) -> Result<QtWindowCompositorDriveStatus> {
    drive_fragment_surface_frame(node_id, target, 0)
}

pub(crate) fn drive_window_compositor_frame_with_drawable(
    node_id: u32,
    target: QtCompositorTarget,
    drawable_handle: u64,
) -> Result<QtWindowCompositorDriveStatus> {
    drive_fragment_surface_frame(node_id, target, drawable_handle)
}

fn velocity_to_desired_fps(velocity: f64) -> f32 {
    // Conservative thresholds — stay at higher fps until velocity is clearly low.
    // Scroll settling lingers at 30-80 px/s, so the 60fps floor must be below that.
    if velocity > 300.0 {
        120.0
    } else if velocity > 20.0 {
        60.0
    } else if velocity > 4.0 {
        30.0
    } else {
        15.0
    }
}

fn drive_fragment_surface_frame(
    node_id: u32,
    target: QtCompositorTarget,
    drawable_handle: u64,
) -> Result<QtWindowCompositorDriveStatus> {
    use crate::canvas::vello::peniko::kurbo::Affine;

    let generation = current_app_generation()?;
    let node = node_by_id(generation, node_id)?;
    ensure_live_node(&node)?;

    // Detect size change — force repaint when viewport resized.
    let prev_target = load_window_compositor_target(node_id);
    let size_changed = prev_target.map_or(true, |prev| {
        prev.width_px != target.width_px || prev.height_px != target.height_px
    });
    store_window_compositor_target(node_id, target);

    // Check whether anything is dirty before doing GPU work.
    let pending = snapshot_window_compositor_pending_state(node_id);
    let has_dirty = !pending.dirty_nodes.is_empty()
        || !pending.dirty_regions.is_empty()
        || !pending.scene_nodes.is_empty()
        || !pending.geometry_nodes.is_empty();
    // Also check if fragment tree has running animations — must not skip tick.
    let has_animation = crate::canvas::fragment::fragment_store_has_animating(node_id);
    if !has_dirty && !size_changed && !has_animation {
        // Release display-link drawable — nothing to render this frame.
        #[cfg(target_os = "macos")]
        if drawable_handle != 0 {
            qt_compositor::release_metal_drawable(drawable_handle);
        }
        return Ok(QtWindowCompositorDriveStatus::Idle);
    }

    let layout = qt::qt_capture_widget_layout(node_id)
        .map_err(|error| qt_error(error.what().to_owned()))?;

    // Run taffy layout before painting so flex children get positioned.
    crate::canvas::fragment::fragment_store_compute_layout(
        node_id,
        f64::from(layout.width_px) / layout.scale_factor,
        f64::from(layout.height_px) / layout.scale_factor,
    );

    let now = crate::qt::trace_now_ns() as f64 / 1_000_000_000.0;
    let (still_animating, completed, max_velocity) =
        crate::canvas::fragment::fragment_store_tick_motion(node_id, now);
    for fid in completed {
        crate::runtime::emit_js_event(crate::api::QtHostEvent::CanvasMotionComplete {
            canvas_node_id: node_id,
            fragment_id: fid.0,
        });
    }

    // Compute dirty rects for partial rendering BEFORE paint, since paint
    // consumes subtree caches and updates dirty_root_children.
    use crate::canvas::fragment::DirtyRectResult;

    let dirty_result = if size_changed {
        DirtyRectResult::FullRepaint
    } else {
        crate::canvas::fragment::fragment_store_compute_dirty_rects(node_id, layout.scale_factor)
    };

    // Extract device-pixel dirty rects for the surface renderer.
    let dirty_rects_device: Option<Vec<(u32, u32, u32, u32)>> = match &dirty_result {
        DirtyRectResult::FullRepaint => None,
        DirtyRectResult::NothingDirty => Some(Vec::new()),
        DirtyRectResult::Partial(rects) => Some(rects.clone()),
    };

    // Check if we have promoted layers and should use the composited path.
    let has_promoted = crate::canvas::fragment::fragment_store_has_promoted(node_id);

    // Convert device-pixel dirty rects to logical coordinates for culling.
    let dirty_clips_logical: Vec<crate::canvas::vello::peniko::kurbo::Rect> = match &dirty_result {
        DirtyRectResult::Partial(rects) => {
            let sf = layout.scale_factor;
            rects.iter().filter_map(|&(dx, dy, dw, dh)| {
                if dw == 0 || dh == 0 { return None; }
                Some(crate::canvas::vello::peniko::kurbo::Rect::new(
                    dx as f64 / sf,
                    dy as f64 / sf,
                    (dx + dw) as f64 / sf,
                    (dy + dh) as f64 / sf,
                ))
            }).collect()
        }
        _ => Vec::new(),
    };
    crate::canvas::fragment::fragment_store_set_dirty_clips(node_id, dirty_clips_logical.clone());

    let presented = if has_promoted {
        // Composited path: build RenderPlan with partitioned layers.
        let render_plan = crate::canvas::fragment::fragment_store_build_render_plan(node_id);
        crate::canvas::fragment::fragment_store_set_dirty_clips(node_id, Vec::new());

        let backdrop_blurs = crate::canvas::fragment::fragment_store_collect_backdrop_blurs(
            node_id, layout.scale_factor,
        );
        let inner_shadows = crate::canvas::fragment::fragment_store_collect_inner_shadows(
            node_id, layout.scale_factor,
        );

        match render_plan {
            Some(plan) => {
                // Promoted path still uses wgpu Surface — release display-link
                // drawable to avoid holding it during get_current_texture().
                #[cfg(target_os = "macos")]
                if drawable_handle != 0 {
                    qt_compositor::release_metal_drawable(drawable_handle);
                }
                crate::surface_renderer::render_composited_and_present(
                    node_id,
                    compositor_target_to_renderer(target).map_err(|e| qt_error(e.to_string()))?,
                    layout.scale_factor,
                    plan,
                    &backdrop_blurs,
                    &inner_shadows,
                    dirty_rects_device.as_deref(),
                )?
            }
            None => true, // No tree → nothing to render.
        }
    } else {
        // Non-promoted path: per-subtree Recording strip caching.
        // Paint per-subtree scenes, then render with strip cache.
        let mut subtrees = crate::canvas::fragment::fragment_store_paint_subtrees(node_id);

        // Clear dirty clips after paint.
        crate::canvas::fragment::fragment_store_set_dirty_clips(node_id, Vec::new());

        // Wrap each subtree with dirty clip layer if partial render.
        if !dirty_clips_logical.is_empty() {
            use anyrender::PaintScene;
            use crate::canvas::vello::peniko::kurbo::{BezPath, Shape};
            let mut clip_path = BezPath::new();
            for rect in &dirty_clips_logical {
                clip_path.extend(rect.path_elements(0.0));
            }
            for (_, sub_scene, dirty) in &mut subtrees {
                if *dirty {
                    let mut wrapped = crate::canvas::vello::Scene::new();
                    wrapped.push_clip_layer(Affine::IDENTITY, &clip_path);
                    wrapped.append_scene(std::mem::take(sub_scene), Affine::IDENTITY);
                    wrapped.pop_layer();
                    *sub_scene = wrapped;
                }
            }
        }

        // Debug overlay: draw dirty rect border with per-frame color.
        // Enable with QT_SOLID_DEBUG_DIRTY=1
        {
            use std::sync::atomic::{AtomicU8, Ordering};
            static ENABLED: AtomicU8 = AtomicU8::new(2); // 2 = unchecked
            let enabled = match ENABLED.load(Ordering::Relaxed) {
                2 => {
                    let v = std::env::var("QT_SOLID_DEBUG_DIRTY").map_or(false, |v| v == "1");
                    ENABLED.store(v as u8, Ordering::Relaxed);
                    v
                }
                v => v == 1,
            };
            if enabled && !dirty_clips_logical.is_empty() {
                use anyrender::PaintScene;
                use crate::canvas::vello::peniko::kurbo::Stroke;
                use crate::canvas::vello::peniko::Color;

                static FRAME_COUNTER: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
                let frame = FRAME_COUNTER.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                let colors = [
                    Color::from_rgba8(255, 0, 0, 128),
                    Color::from_rgba8(0, 255, 0, 128),
                    Color::from_rgba8(0, 128, 255, 128),
                    Color::from_rgba8(255, 255, 0, 128),
                    Color::from_rgba8(255, 0, 255, 128),
                    Color::from_rgba8(0, 255, 255, 128),
                ];
                // Append debug overlay to last subtree (or create one).
                let debug_scene = if let Some((_, last, dirty)) = subtrees.last_mut() {
                    *dirty = true; // Force re-record since we modified it.
                    last
                } else {
                    subtrees.push((crate::canvas::fragment::FragmentId(u32::MAX - 1), crate::canvas::vello::Scene::new(), true));
                    &mut subtrees.last_mut().unwrap().1
                };
                for (i, clip_rect) in dirty_clips_logical.iter().enumerate() {
                    let color = colors[(frame as usize + i) % colors.len()];
                    let fill_color = Color::from_rgba8(
                        color.to_rgba8().r, color.to_rgba8().g, color.to_rgba8().b, 24,
                    );
                    debug_scene.fill(peniko::Fill::NonZero, Affine::IDENTITY, fill_color, None, clip_rect);
                    debug_scene.stroke(&Stroke::new(2.0), Affine::IDENTITY, color, None, clip_rect);
                }
            }
        }

        let backdrop_blurs = crate::canvas::fragment::fragment_store_collect_backdrop_blurs(
            node_id, layout.scale_factor,
        );
        let inner_shadows = crate::canvas::fragment::fragment_store_collect_inner_shadows(
            node_id, layout.scale_factor,
        );

        crate::surface_renderer::render_and_present_subtrees(
            node_id,
            compositor_target_to_renderer(target).map_err(|e| qt_error(e.to_string()))?,
            layout.scale_factor,
            subtrees,
            &backdrop_blurs,
            &inner_shadows,
            dirty_rects_device.as_deref(),
            drawable_handle,
        )?
    };

    if !presented {
        // Surface not ready — preserve dirty state so next frame drive retries.
        return Ok(QtWindowCompositorDriveStatus::Busy);
    }

    // Consume dirty state after successful present.
    crate::canvas::fragment::fragment_store_consume_dirty_state(node_id);

    if still_animating {
        let desired_fps = velocity_to_desired_fps(max_velocity);
        crate::qt::ffi::bridge::qt_macos_set_display_link_frame_rate(node_id, desired_fps);
        crate::runtime::request_overlay_next_frame_exact(&node, node_id)?;
    }

    clear_window_compositor_dirty_nodes(node_id);

    Ok(QtWindowCompositorDriveStatus::Presented)
}

pub(crate) fn window_motion_hit_test(
    window_id: u32,
    screen_x: i32,
    screen_y: i32,
) -> Result<QtMotionMouseTarget> {
    let generation = current_app_generation()?;
    let _node = node_by_id(generation, window_id)?;
    let bounds = debug_node_bounds(window_id)?;
    let local_x = f64::from(screen_x - bounds.screen_x);
    let local_y = f64::from(screen_y - bounds.screen_y);

    let hit = crate::canvas::fragment::fragment_store_hit_test(window_id, local_x, local_y);
    match hit {
        Some(fid) => Ok(QtMotionMouseTarget {
            found: true,
            root_node_id: fid.0,
            local_x,
            local_y,
        }),
        None => Ok(QtMotionMouseTarget {
            found: false,
            root_node_id: 0,
            local_x: 0.0,
            local_y: 0.0,
        }),
    }
}

pub(crate) fn window_motion_map_point_to_root(
    window_id: u32,
    root_node_id: u32,
    screen_x: i32,
    screen_y: i32,
) -> Result<QtMotionMouseTarget> {
    let bounds = debug_node_bounds(window_id)?;
    let local_x = f64::from(screen_x - bounds.screen_x);
    let local_y = f64::from(screen_y - bounds.screen_y);
    Ok(QtMotionMouseTarget {
        found: true,
        root_node_id,
        local_x,
        local_y,
    })
}

pub(crate) fn window_motion_hit_root_ids(_window_id: u32) -> Result<Vec<u32>> {
    // TODO: implement motion root enumeration once fragment store supports it.
    Ok(Vec::new())
}
