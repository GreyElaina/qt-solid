pub mod decl;
mod encode;
mod hit_test;
mod kinds;
mod node;
mod paint;
mod parse;
mod tree;
mod types;

pub use kinds::*;
pub use node::*;
pub use parse::*;
pub use tree::*;
pub use types::*;

// ---------------------------------------------------------------------------
// Fragment store — delegated to runtime state (per-window FragmentTree)
// ---------------------------------------------------------------------------

use std::collections::HashMap;

use taffy::prelude::*;

use decl::FragmentValue;
use super::vello::peniko::kurbo::{Affine, BezPath, Point, Rect, Vec2};
use super::vello::peniko::{Color, ImageData};
use super::vello::Scene;
use crate::runtime;
use crate::scene_renderer::effect_pass::{BackdropBlurEffect, InnerShadowEffect};

pub fn fragment_store_ensure(canvas_node_id: u32) {
    runtime::ensure_fragment_tree(canvas_node_id);
}

pub fn fragment_store_remove(canvas_node_id: u32) {
    runtime::remove_fragment_tree(canvas_node_id);
}

pub fn fragment_store_create_node(canvas_node_id: u32, tag: &str) -> Option<FragmentId> {
    let kind = FragmentData::from_tag_loose(tag)?;
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.create_node(kind))
}

pub fn fragment_store_insert_child(
    canvas_node_id: u32,
    parent: Option<FragmentId>,
    child: FragmentId,
    before: Option<FragmentId>,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.insert_child(parent, child, before);
        // Invalidate parent text shaped cache when a span is inserted.
        if let Some(parent_id) = parent {
            if let Some(child_node) = tree.nodes.get(&child) {
                if matches!(child_node.kind, FragmentData::Span(_)) {
                    if let Some(parent_node) = tree.nodes.get_mut(&parent_id) {
                        if let FragmentData::Text(ref mut t) = parent_node.kind {
                            t.shaped = None;
                            parent_node.dirty = true;
                        }
                    }
                }
            }
        }
    });
}

pub fn fragment_store_detach_child(
    canvas_node_id: u32,
    parent: Option<FragmentId>,
    child: FragmentId,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        let is_span = tree.nodes.get(&child).map_or(false, |n| matches!(n.kind, FragmentData::Span(_)));
        tree.detach_child(parent, child);
        if is_span {
            if let Some(parent_id) = parent {
                if let Some(parent_node) = tree.nodes.get_mut(&parent_id) {
                    if let FragmentData::Text(ref mut t) = parent_node.kind {
                        t.shaped = None;
                        parent_node.dirty = true;
                    }
                }
            }
        }
    });
}

pub fn fragment_store_destroy(canvas_node_id: u32, fragment_id: FragmentId) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.remove(fragment_id);
    });
}

pub fn fragment_store_set_image_data(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    image_data: ImageData,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::Image(ref mut img) = node.kind {
                img.image_data = Some(image_data);
                node.dirty = true;
                tree.any_dirty = true;
                tree.cached_scene = None;
            }
        }
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_clear_image_data(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::Image(ref mut img) = node.kind {
                img.image_data = None;
                node.dirty = true;
                tree.any_dirty = true;
                tree.cached_scene = None;
            }
        }
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_set_prop(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    key: &str,
    value: FragmentValue,
) -> bool {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if is_layout_prop(key) {
            if let FragmentValue::F64 { value } = &value {
                let v = *value as f32;
                tree.with_taffy_style_mut(fragment_id, |style| {
                    apply_layout_prop_to_style(style, key, v);
                });
            } else if let FragmentValue::Str { ref value } = value {
                tree.with_taffy_style_mut(fragment_id, |style| {
                    apply_layout_string_prop_to_style(style, key, value);
                });
            } else if let FragmentValue::GridTracks { ref tracks } = value {
                tree.with_taffy_style_mut(fragment_id, |style| {
                    apply_grid_tracks_to_style(style, key, tracks);
                });
            }
            tree.any_dirty = true;
            tree.aabbs_dirty = true;
            tree.cached_scene = None;
            tree.invalidate_subtree_cache_for(fragment_id);
            return;
        }

        // Track promoted_node_count for "layer" prop changes.
        if key == "layer" {
            if let FragmentValue::Bool { value } = &value {
                let was_promoted = tree.nodes.get(&fragment_id).map_or(false, |n| n.promoted);
                if *value && !was_promoted {
                    tree.promoted_node_count += 1;
                } else if !*value && was_promoted {
                    tree.promoted_node_count = tree.promoted_node_count.saturating_sub(1);
                    tree.promoted_scene_cache.remove(&fragment_id);
                }
            }
        }

        // Track explicit width/height and sync taffy size (before value is moved).
        if key == "width" || key == "height" {
            if let FragmentValue::F64 { value: v } = &value {
                let fv = *v;
                if let Some(node) = tree.nodes.get_mut(&fragment_id) {
                    if key == "width" {
                        node.props.explicit_width = if fv > 0.0 { Some(fv) } else { None };
                    } else {
                        node.props.explicit_height = if fv > 0.0 { Some(fv) } else { None };
                    }
                }
                let v32 = fv as f32;
                tree.with_taffy_style_mut(fragment_id, |style| {
                    if key == "width" {
                        style.size.width = if v32 > 0.0 { taffy::style::Dimension::length(v32) } else { taffy::style::Dimension::auto() };
                    } else {
                        style.size.height = if v32 > 0.0 { taffy::style::Dimension::length(v32) } else { taffy::style::Dimension::auto() };
                    }
                });
            } else if let FragmentValue::Str { ref value } = value {
                if let Some(dim) = parse_dimension_string(value) {
                    if let Some(node) = tree.nodes.get_mut(&fragment_id) {
                        if key == "width" { node.props.explicit_width = None; } else { node.props.explicit_height = None; }
                    }
                    tree.with_taffy_style_mut(fragment_id, |style| {
                        if key == "width" { style.size.width = dim; } else { style.size.height = dim; }
                    });
                    tree.any_dirty = true;
                    tree.cached_scene = None;
                }
            }
        }

        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            apply_fragment_prop(node, key, value);
            node.dirty = true;
            tree.any_dirty = true;
            tree.cached_scene = None;
        }
        // When a span child changes, invalidate parent text shaped cache.
        if let Some(node) = tree.nodes.get(&fragment_id) {
            if matches!(node.kind, FragmentData::Span(_)) {
                if let Some(parent_id) = node.parent {
                    if let Some(parent) = tree.nodes.get_mut(&parent_id) {
                        if let FragmentData::Text(ref mut t) = parent.kind {
                            t.shaped = None;
                            parent.dirty = true;
                        }
                    }
                }
            }
        }
        tree.invalidate_subtree_cache_for(fragment_id);

        // Sync taffy position mode when x/y explicit state changes.
        if key == "x" || key == "y" {
            let (ex, ey) = tree.nodes.get(&fragment_id)
                .map(|n| (n.props.explicit_x, n.props.explicit_y))
                .unwrap_or((None, None));
            tree.with_taffy_style_mut(fragment_id, |style| {
                if ex.is_some() || ey.is_some() {
                    style.position = taffy::style::Position::Absolute;
                    style.inset.left = ex
                        .map(|v| taffy::style::LengthPercentageAuto::length(v as f32))
                        .unwrap_or(taffy::style::LengthPercentageAuto::auto());
                    style.inset.top = ey
                        .map(|v| taffy::style::LengthPercentageAuto::length(v as f32))
                        .unwrap_or(taffy::style::LengthPercentageAuto::auto());
                } else {
                    style.position = taffy::style::Position::Relative;
                    style.inset.left = taffy::style::LengthPercentageAuto::auto();
                    style.inset.top = taffy::style::LengthPercentageAuto::auto();
                }
            });
        }
    })
    .is_some()
}

pub fn fragment_store_set_dirty_clips(canvas_node_id: u32, clips: Vec<Rect>) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.dirty_clips = clips;
    });
}

pub fn fragment_store_paint(canvas_node_id: u32, scene: &mut Scene, transform: Affine) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.paint_into_scene(scene, transform);
    });
}

pub fn fragment_store_paint_subtrees(canvas_node_id: u32) -> Vec<(FragmentId, Scene, bool)> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.paint_subtrees()
    }).unwrap_or_default()
}

pub fn fragment_store_compute_dirty_rects(canvas_node_id: u32, scale_factor: f64) -> DirtyRectResult {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.compute_dirty_rects(scale_factor)
    }).unwrap_or(DirtyRectResult::FullRepaint)
}

pub fn fragment_store_consume_dirty_state(canvas_node_id: u32) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.consume_dirty_state();
    });
}

pub fn fragment_store_force_full_repaint(canvas_node_id: u32) -> bool {
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.force_full_repaint)
        .unwrap_or(true)
}

pub fn fragment_store_paint_single(canvas_node_id: u32, fragment_id: FragmentId, scene: &mut Scene, transform: Affine) {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.paint_node_self_only(scene, fragment_id, transform);
    });
}

pub fn fragment_store_paint_at_origin(canvas_node_id: u32, fragment_id: FragmentId, scene: &mut Scene, transform: Affine) {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.paint_node_at_origin(scene, fragment_id, transform);
    });
}

pub fn fragment_store_world_bounds(canvas_node_id: u32, fragment_id: FragmentId) -> Option<Rect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.ensure_aabbs();
        tree.node(fragment_id)?.world_aabb
    }).flatten()
}

pub fn fragment_store_build_paint_plan(canvas_node_id: u32) -> Option<PaintPlan> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.build_paint_plan())
}

pub fn fragment_store_build_render_plan(canvas_node_id: u32) -> Option<RenderPlan> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.build_paint_plan().partition()
    })
}

pub fn fragment_store_has_promoted(canvas_node_id: u32) -> bool {
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.has_promoted_nodes())
        .unwrap_or(false)
}

pub fn fragment_store_collect_inner_shadows(canvas_node_id: u32, scale_factor: f64) -> Vec<InnerShadowEffect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.collect_inner_shadow_effects(scale_factor)
    })
    .unwrap_or_default()
}

pub fn fragment_store_collect_backdrop_blurs(canvas_node_id: u32, scale_factor: f64) -> Vec<BackdropBlurEffect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.collect_backdrop_blur_effects(scale_factor)
    })
    .unwrap_or_default()
}

pub fn fragment_store_has_animating(canvas_node_id: u32) -> bool {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.nodes.values().any(|n| {
            n.timeline.as_ref().map_or(false, |t| t.is_animating())
        })
    })
    .unwrap_or(false)
}

pub fn fragment_store_hit_test(canvas_node_id: u32, x: f64, y: f64) -> Option<FragmentId> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.hit_test((x, y)))?
}

pub fn fragment_store_set_debug_highlight(canvas_node_id: u32, fragment_id: Option<FragmentId>) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.set_debug_highlight(fragment_id);
    });
}

pub fn fragment_store_get_cursor(canvas_node_id: u32, fragment_id: FragmentId) -> u8 {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.nodes.get(&fragment_id).map_or(0, |n| n.props.cursor)
    })
    .unwrap_or(0)
}

/// Move focus to the next/previous focusable fragment.
pub fn fragment_store_focus_next(canvas_node_id: u32, forward: bool) -> (i32, i32) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        let (old, new) = tree.focus_next(forward);
        (
            old.map(|id| id.0 as i32).unwrap_or(-1),
            new.map(|id| id.0 as i32).unwrap_or(-1),
        )
    })
    .unwrap_or((-1, -1))
}

/// Focus a specific fragment by click.
pub fn fragment_store_focus_fragment(canvas_node_id: u32, fragment_id: FragmentId) -> (i32, i32) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        let old = tree.focus_fragment(fragment_id);
        let new_focused = tree.focused().map(|id| id.0 as i32).unwrap_or(-1);
        (
            old.map(|id| id.0 as i32).unwrap_or(-1),
            new_focused,
        )
    })
    .unwrap_or((-1, -1))
}

pub fn fragment_store_focused(canvas_node_id: u32) -> i32 {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.focused().map(|id| id.0 as i32).unwrap_or(-1)
    })
    .unwrap_or(-1)
}

pub fn fragment_store_set_text_shape_cache(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    cache: ShapedTextCache,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::Text(ref mut text) = node.kind {
                text.shaped = Some(cache);
            }
            node.dirty = true;
        }
        tree.any_dirty = true;
        tree.cached_scene = None;
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_set_text_input_layout_cache(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    layout: ShapedTextLayout,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::TextInput(ref mut ti) = node.kind {
                ti.layout = Some(layout);
            }
            node.dirty = true;
        }
        tree.any_dirty = true;
        tree.cached_scene = None;
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_set_text_input_state(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    text: String,
    cursor_pos: f64,
    selection_anchor: f64,
    layout: ShapedTextLayout,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::TextInput(ref mut ti) = node.kind {
                ti.text = text;
                ti.cursor_pos = cursor_pos;
                ti.selection_anchor = selection_anchor;
                ti.layout = Some(layout);
            }
            node.dirty = true;
        }
        tree.any_dirty = true;
        tree.cached_scene = None;
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_set_caret_visible(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    visible: bool,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::TextInput(ref mut ti) = node.kind {
                ti.caret_visible = visible;
            }
            node.dirty = true;
        }
        tree.any_dirty = true;
        tree.cached_scene = None;
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_read_text_props(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<(String, f64, String, i32, bool, f64, String)> {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        let node = tree.node(fragment_id)?;
        if let FragmentData::Text(ref text) = node.kind {
            if text.shaped.is_some() {
                return None;
            }
            let weight = text.font_weight as i32;
            let italic = text.font_style == "italic";
            Some((text.text.clone(), text.font_size, text.font_family.clone(), weight, italic, text.text_max_width, text.text_overflow.clone()))
        } else {
            None
        }
    })?
}

/// Collect styled text runs from span children of a text fragment.
pub fn fragment_store_read_text_style_runs(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<(Vec<TextStyleRun>, f64, String, f64, String)> {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        let node = tree.node(fragment_id)?;
        let text_frag = match &node.kind {
            FragmentData::Text(t) => t,
            _ => return None,
        };
        if text_frag.shaped.is_some() {
            return None;
        }
        let children = &node.children;
        if children.is_empty() {
            return None;
        }
        let mut runs = Vec::new();
        for &child_id in children {
            let child = tree.node(child_id)?;
            if let FragmentData::Span(ref span) = child.kind {
                runs.push(TextStyleRun {
                    text: span.text.clone(),
                    font_size: if span.font_size > 0.0 { span.font_size } else { text_frag.font_size },
                    font_family: if span.font_family.is_empty() { text_frag.font_family.clone() } else { span.font_family.clone() },
                    font_weight: if span.font_weight > 0.0 { span.font_weight as i32 } else { text_frag.font_weight as i32 },
                    font_italic: if span.font_style.is_empty() { text_frag.font_style == "italic" } else { span.font_style == "italic" },
                    color: span.color,
                });
            }
        }
        if runs.is_empty() {
            None
        } else {
            Some((runs, text_frag.font_size, text_frag.font_family.clone(), text_frag.text_max_width, text_frag.text_overflow.clone()))
        }
    })?
}

/// If the given fragment is a Span child of a Text parent, return the parent Text id.
pub fn fragment_store_parent_text_for_span(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<FragmentId> {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        let node = tree.node(fragment_id)?;
        if !matches!(node.kind, FragmentData::Span(_)) {
            return None;
        }
        let parent_id = node.parent?;
        let parent = tree.node(parent_id)?;
        if matches!(parent.kind, FragmentData::Text(_)) {
            Some(parent_id)
        } else {
            None
        }
    }).flatten()
}

/// Read text input props for reshaping.
pub fn fragment_store_read_text_input_props(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<(String, f64, String, i32, bool)> {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        let node = tree.node(fragment_id)?;
        if let FragmentData::TextInput(ref ti) = node.kind {
            if ti.layout.is_some() {
                return None;
            }
            let weight = ti.font_weight as i32;
            let italic = ti.font_style == "italic";
            Some((ti.text.clone(), ti.font_size, ti.font_family.clone(), weight, italic))
        } else {
            None
        }
    })?
}

pub fn fragment_store_click_to_cursor(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    window_x: f64,
    _window_y: f64,
) {
    let local_x = runtime::with_fragment_tree(canvas_node_id, |tree| {
        let node = tree.node(fragment_id)?;
        if !matches!(node.kind, FragmentData::TextInput(_)) {
            return None;
        }
        let world = tree.world_transform(fragment_id);
        let inv = world.inverse();
        let local = inv * Point::new(window_x, 0.0);
        Some(local.x)
    }).flatten();

    if let Some(lx) = local_x {
        let _ = crate::qt::ffi::qt_text_edit_click_to_cursor(canvas_node_id, lx);
    }
}

pub fn fragment_store_drag_to_cursor(
    canvas_node_id: u32,
    window_x: f64,
    _window_y: f64,
) {
    let local_x = runtime::with_fragment_tree(canvas_node_id, |tree| {
        let focused_id = tree.focused()?;
        let node = tree.node(focused_id)?;
        if !matches!(node.kind, FragmentData::TextInput(_)) {
            return None;
        }
        let world = tree.world_transform(focused_id);
        let inv = world.inverse();
        let local = inv * Point::new(window_x, 0.0);
        Some(local.x)
    }).flatten();

    if let Some(lx) = local_x {
        let _ = crate::qt::ffi::qt_text_edit_drag_to_cursor(canvas_node_id, lx);
    }
}

pub fn fragment_store_mark_dirty(canvas_node_id: u32, fragment_id: FragmentId) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.mark_dirty(fragment_id);
    });
}

pub fn fragment_store_compute_layout(
    canvas_node_id: u32,
    available_width: f64,
    available_height: f64,
) {
    let events = runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.compute_layout(available_width, available_height)
    });
    if let Some(events) = events {
        for event in events {
            runtime::emit_js_event(crate::api::QtHostEvent::FragmentLayout {
                canvas_node_id,
                fragment_id: event.fragment_id.0,
                x: event.x,
                y: event.y,
                width: event.width,
                height: event.height,
            });
        }
    }
}

pub fn fragment_store_set_listener(
    canvas_node_id: u32,
    fragment_id: u32,
    listener_bit: u32,
    enabled: bool,
) {
    let id = FragmentId(fragment_id);
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&id) {
            let flags = FragmentListeners::from_bits_truncate(listener_bit);
            if enabled {
                node.listeners.insert(flags);
            } else {
                node.listeners.remove(flags);
            }
        }
    });
}

pub fn fragment_store_set_motion_target(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    targets: &[(motion::PropertyKey, f64)],
    default_transition: &motion::TransitionSpec,
    per_property: &std::collections::HashMap<motion::PropertyKey, motion::TransitionSpec>,
    delay_secs: f64,
    now: f64,
) -> bool {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.set_motion_target(fragment_id, targets, default_transition, per_property, delay_secs, now)
    })
    .unwrap_or(false)
}

pub fn fragment_store_set_motion_target_keyframes(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    targets: Vec<(motion::PropertyKey, Vec<f64>)>,
    times: Option<Vec<f64>>,
    default_transition: &motion::TransitionSpec,
    per_property: &std::collections::HashMap<motion::PropertyKey, motion::TransitionSpec>,
    delay_secs: f64,
    now: f64,
) -> bool {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.set_motion_target_keyframes(fragment_id, targets, times, default_transition, per_property, delay_secs, now)
    })
    .unwrap_or(false)
}

pub fn fragment_store_tick_motion(canvas_node_id: u32, now: f64) -> (bool, Vec<FragmentId>, f64) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.tick_motion(now))
        .unwrap_or((false, Vec::new(), 0.0))
}

pub fn fragment_store_get_world_bounds(canvas_node_id: u32, fragment_id: FragmentId) -> Option<Rect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.get_world_bounds(fragment_id))
        .flatten()
}

pub fn fragment_store_set_scroll_offset(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    x: f64,
    y: f64,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.set_scroll_offset(fragment_id, Vec2::new(x, y));
    });
}

pub fn fragment_store_drive_scroll_motion(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    x: f64,
    y: f64,
    now: f64,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.drive_scroll_motion(fragment_id, x, y, now);
    });
}

pub fn fragment_store_release_scroll_motion(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    clamped_x: f64,
    clamped_y: f64,
    spring: motion::TransitionSpec,
    now: f64,
) -> bool {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.release_scroll_motion(fragment_id, clamped_x, clamped_y, spring, now)
    })
    .unwrap_or(false)
}

pub fn fragment_store_get_content_size(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<(f64, f64)> {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.get_content_size(fragment_id)
    }).flatten()
}

pub fn fragment_store_set_layout_flip(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    dx: f64,
    dy: f64,
    sx: f64,
    sy: f64,
    transition: &motion::TransitionSpec,
    now: f64,
) -> bool {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.set_layout_flip(fragment_id, dx, dy, sx, sy, transition, now)
    })
    .unwrap_or(false)
}

// ---------------------------------------------------------------------------
// Layout prop keys
// ---------------------------------------------------------------------------

const LAYOUT_PROPS: &[&str] = &[
    "display",
    "flexDirection", "flexGrow", "flexShrink", "flexBasis",
    "flexWrap", "alignItems", "alignSelf", "justifyContent",
    "gap", "padding", "paddingTop", "paddingRight", "paddingBottom", "paddingLeft",
    "margin", "marginTop", "marginRight", "marginBottom", "marginLeft",
    "minWidth", "minHeight", "maxWidth", "maxHeight",
    "position", "overflow", "overflowX", "overflowY",
    "gridTemplateRows", "gridTemplateColumns",
    "gridAutoFlow", "gridRow", "gridColumn", "gridRowSpan", "gridColSpan",
];

fn is_layout_prop(key: &str) -> bool {
    LAYOUT_PROPS.contains(&key)
}

/// Parse dimension strings like "100%", "50%", "auto".
fn parse_dimension_string(s: &str) -> Option<taffy::style::Dimension> {
    let s = s.trim();
    if s == "auto" {
        return Some(taffy::style::Dimension::auto());
    }
    if let Some(pct) = s.strip_suffix('%') {
        if let Ok(v) = pct.trim().parse::<f32>() {
            return Some(taffy::style::Dimension::percent(v / 100.0));
        }
    }
    None
}

fn apply_layout_prop_to_style(style: &mut taffy::Style, key: &str, v: f32) {
    match key {
        "flexGrow" => style.flex_grow = v,
        "flexShrink" => style.flex_shrink = v,
        "flexBasis" => style.flex_basis = taffy::style::Dimension::length(v),
        "gap" => {
            style.gap = taffy::geometry::Size {
                width: taffy::style::LengthPercentage::length(v),
                height: taffy::style::LengthPercentage::length(v),
            };
        }
        "padding" => {
            let lp = taffy::style::LengthPercentage::length(v);
            style.padding = taffy::geometry::Rect { top: lp, right: lp, bottom: lp, left: lp };
        }
        "margin" => {
            let lpa = taffy::style::LengthPercentageAuto::length(v);
            style.margin = taffy::geometry::Rect { top: lpa, right: lpa, bottom: lpa, left: lpa };
        }
        "paddingTop" => style.padding.top = taffy::style::LengthPercentage::length(v),
        "paddingRight" => style.padding.right = taffy::style::LengthPercentage::length(v),
        "paddingBottom" => style.padding.bottom = taffy::style::LengthPercentage::length(v),
        "paddingLeft" => style.padding.left = taffy::style::LengthPercentage::length(v),
        "marginTop" => style.margin.top = taffy::style::LengthPercentageAuto::length(v),
        "marginRight" => style.margin.right = taffy::style::LengthPercentageAuto::length(v),
        "marginBottom" => style.margin.bottom = taffy::style::LengthPercentageAuto::length(v),
        "marginLeft" => style.margin.left = taffy::style::LengthPercentageAuto::length(v),
        "minWidth" => style.min_size.width = taffy::style::Dimension::length(v),
        "minHeight" => style.min_size.height = taffy::style::Dimension::length(v),
        "maxWidth" => style.max_size.width = taffy::style::Dimension::length(v),
        "maxHeight" => style.max_size.height = taffy::style::Dimension::length(v),
        "gridRow" => {
            let idx = v as i16;
            style.grid_row = taffy::geometry::Line {
                start: GridPlacement::from_line_index(idx),
                end: GridPlacement::Auto,
            };
        }
        "gridColumn" => {
            let idx = v as i16;
            style.grid_column = taffy::geometry::Line {
                start: GridPlacement::from_line_index(idx),
                end: GridPlacement::Auto,
            };
        }
        "gridRowSpan" => {
            let s = (v as u16).max(1);
            style.grid_row = taffy::geometry::Line {
                start: style.grid_row.start.clone(),
                end: GridPlacement::Span(s),
            };
        }
        "gridColSpan" => {
            let s = (v as u16).max(1);
            style.grid_column = taffy::geometry::Line {
                start: style.grid_column.start.clone(),
                end: GridPlacement::Span(s),
            };
        }
        _ => {}
    }
}

fn apply_layout_string_prop_to_style(style: &mut taffy::Style, key: &str, v: &str) {
    match key {
        "display" => {
            style.display = match v {
                "flex" => taffy::style::Display::Flex,
                "grid" => taffy::style::Display::Grid,
                "none" => taffy::style::Display::None,
                _ => taffy::style::Display::Flex,
            };
        }
        "gridAutoFlow" => {
            style.grid_auto_flow = match v {
                "row" => GridAutoFlow::Row,
                "column" => GridAutoFlow::Column,
                "row-dense" => GridAutoFlow::RowDense,
                "column-dense" => GridAutoFlow::ColumnDense,
                _ => GridAutoFlow::Row,
            };
        }
        "flexDirection" => {
            style.flex_direction = match v {
                "row" => taffy::style::FlexDirection::Row,
                "column" => taffy::style::FlexDirection::Column,
                "row-reverse" => taffy::style::FlexDirection::RowReverse,
                "column-reverse" => taffy::style::FlexDirection::ColumnReverse,
                _ => taffy::style::FlexDirection::Column,
            };
        }
        "flexWrap" => {
            style.flex_wrap = match v {
                "nowrap" => taffy::style::FlexWrap::NoWrap,
                "wrap" => taffy::style::FlexWrap::Wrap,
                "wrap-reverse" => taffy::style::FlexWrap::WrapReverse,
                _ => taffy::style::FlexWrap::NoWrap,
            };
        }
        "alignItems" => {
            style.align_items = match v {
                "flex-start" | "start" => Some(taffy::style::AlignItems::FlexStart),
                "flex-end" | "end" => Some(taffy::style::AlignItems::FlexEnd),
                "center" => Some(taffy::style::AlignItems::Center),
                "stretch" => Some(taffy::style::AlignItems::Stretch),
                "baseline" => Some(taffy::style::AlignItems::Baseline),
                _ => None,
            };
        }
        "alignSelf" => {
            style.align_self = match v {
                "flex-start" | "start" => Some(taffy::style::AlignSelf::FlexStart),
                "flex-end" | "end" => Some(taffy::style::AlignSelf::FlexEnd),
                "center" => Some(taffy::style::AlignSelf::Center),
                "stretch" => Some(taffy::style::AlignSelf::Stretch),
                _ => None,
            };
        }
        "justifyContent" => {
            style.justify_content = match v {
                "flex-start" | "start" => Some(taffy::style::JustifyContent::FlexStart),
                "flex-end" | "end" => Some(taffy::style::JustifyContent::FlexEnd),
                "center" => Some(taffy::style::JustifyContent::Center),
                "space-between" => Some(taffy::style::JustifyContent::SpaceBetween),
                "space-around" => Some(taffy::style::JustifyContent::SpaceAround),
                "space-evenly" => Some(taffy::style::JustifyContent::SpaceEvenly),
                _ => None,
            };
        }
        "position" => {
            style.position = match v {
                "relative" => taffy::style::Position::Relative,
                "absolute" => taffy::style::Position::Absolute,
                _ => taffy::style::Position::Relative,
            };
        }
        "overflow" => {
            let ov = match v {
                "visible" => taffy::style::Overflow::Visible,
                "clip" => taffy::style::Overflow::Clip,
                "hidden" => taffy::style::Overflow::Hidden,
                "scroll" => taffy::style::Overflow::Scroll,
                _ => taffy::style::Overflow::Visible,
            };
            style.overflow = taffy::geometry::Point { x: ov, y: ov };
        }
        "overflowX" => {
            style.overflow.x = match v {
                "visible" => taffy::style::Overflow::Visible,
                "clip" => taffy::style::Overflow::Clip,
                "hidden" => taffy::style::Overflow::Hidden,
                "scroll" => taffy::style::Overflow::Scroll,
                _ => taffy::style::Overflow::Visible,
            };
        }
        "overflowY" => {
            style.overflow.y = match v {
                "visible" => taffy::style::Overflow::Visible,
                "clip" => taffy::style::Overflow::Clip,
                "hidden" => taffy::style::Overflow::Hidden,
                "scroll" => taffy::style::Overflow::Scroll,
                _ => taffy::style::Overflow::Visible,
            };
        }
        _ => {}
    }
}

fn parse_track_size(s: &str) -> GridTemplateComponent<String> {
    let s = s.trim();
    if s == "auto" {
        return GridTemplateComponent::AUTO;
    }
    if s == "min-content" {
        return GridTemplateComponent::MIN_CONTENT;
    }
    if s == "max-content" {
        return GridTemplateComponent::MAX_CONTENT;
    }
    if let Some(fr_str) = s.strip_suffix("fr") {
        if let Ok(v) = fr_str.trim().parse::<f32>() {
            return GridTemplateComponent::from_fr(v);
        }
    }
    if let Some(pct_str) = s.strip_suffix('%') {
        if let Ok(v) = pct_str.trim().parse::<f32>() {
            return GridTemplateComponent::from_percent(v / 100.0);
        }
    }
    if let Ok(v) = s.parse::<f32>() {
        return GridTemplateComponent::from_length(v);
    }
    GridTemplateComponent::AUTO
}

fn apply_grid_tracks_to_style(style: &mut taffy::Style, key: &str, tracks: &[String]) {
    let parsed: Vec<GridTemplateComponent<String>> = tracks.iter().map(|t| parse_track_size(t)).collect();
    match key {
        "gridTemplateRows" => {
            style.display = taffy::style::Display::Grid;
            style.grid_template_rows = parsed;
        }
        "gridTemplateColumns" => {
            style.display = taffy::style::Display::Grid;
            style.grid_template_columns = parsed;
        }
        _ => {}
    }
}

fn apply_fragment_prop(node: &mut FragmentNode, key: &str, value: FragmentValue) {
    match key {
        "x" => {
            match value {
                FragmentValue::F64 { value } => { node.props.explicit_x = Some(value); }
                FragmentValue::Unset => { node.props.explicit_x = None; }
                _ => {}
            }
            return;
        }
        "y" => {
            match value {
                FragmentValue::F64 { value } => { node.props.explicit_y = Some(value); }
                FragmentValue::Unset => { node.props.explicit_y = None; }
                _ => {}
            }
            return;
        }
        "opacity" => {
            if let FragmentValue::F64 { value } = value { node.props.opacity = value as f32; }
            return;
        }
        "clip" => {
            if let FragmentValue::Bool { value } = value { node.props.clip = value; }
            return;
        }
        "clipPath" => {
            match value {
                FragmentValue::Str { ref value } => {
                    node.props.clip_path = if value.is_empty() {
                        None
                    } else {
                        BezPath::from_svg(value).ok()
                    };
                }
                FragmentValue::Unset => { node.props.clip_path = None; }
                _ => {}
            }
            return;
        }
        "visible" => {
            if let FragmentValue::Bool { value } = value { node.props.visible = value; }
            return;
        }
        "pointerEvents" => {
            if let FragmentValue::Bool { value } = value { node.props.pointer_events = value; }
            return;
        }
        "cursor" => {
            if let FragmentValue::Str { ref value } = value {
                node.props.cursor = match value.as_str() {
                    "pointer" | "hand" => 1,
                    "text" | "ibeam" => 2,
                    "crosshair" => 3,
                    "move" => 4,
                    "wait" => 5,
                    "not-allowed" | "forbidden" => 6,
                    "grab" => 7,
                    "grabbing" => 8,
                    _ => 0,
                };
            }
            return;
        }
        "focusable" => {
            if let FragmentValue::Bool { value } = value { node.props.focusable = value; }
            return;
        }
        "layer" => {
            if let FragmentValue::Bool { value } = value { node.promoted = value; }
            return;
        }
        "blendMode" => {
            if let FragmentValue::BlendMode { value } = value {
                node.props.blend_mode = value.into();
            }
            return;
        }
        "backdropBlur" => {
            match value {
                FragmentValue::F64 { value } => {
                    node.props.backdrop_blur = if value > 0.0 { Some(value) } else { None };
                }
                FragmentValue::Unset => { node.props.backdrop_blur = None; }
                _ => {}
            }
            return;
        }
        "zIndex" => {
            match value {
                FragmentValue::F64 { value } => { node.props.z_index = value as i32; }
                FragmentValue::Unset => { node.props.z_index = 0; }
                _ => {}
            }
            return;
        }
        _ => {}
    }
    if matches!(value, FragmentValue::Unset) {
        node.kind.reset_prop(key);
        return;
    }
    node.kind.apply_prop(key, value);
    if matches!(key, "text" | "fontSize" | "fontFamily" | "fontWeight" | "fontStyle" | "textMaxWidth" | "textOverflow") {
        match &mut node.kind {
            FragmentData::Text(t) => { t.shaped = None; }
            FragmentData::TextInput(ti) => { ti.layout = None; }
            _ => {}
        }
    }
    if key == "d" {
        if let FragmentData::Path(ref mut path) = node.kind {
            path.reparse_path();
        }
    }
}

// ---------------------------------------------------------------------------
// Devtools snapshot
// ---------------------------------------------------------------------------

#[derive(Debug, Clone)]
pub struct FragmentNodeSnapshot {
    pub id: u32,
    pub tag: String,
    pub parent_id: Option<u32>,
    pub child_ids: Vec<u32>,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub clip: bool,
    pub visible: bool,
    pub opacity: f32,
    pub props: HashMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct LayerSnapshot {
    pub fragment_id: u32,
    pub layer_key: u32,
    pub x: f64,
    pub y: f64,
    pub width: f64,
    pub height: f64,
    pub opacity: f32,
    pub reasons: String,
}

#[derive(Debug, Clone)]
pub struct AnimationChannelSnapshot {
    pub property: String,
    pub origin: f64,
    pub target: f64,
    pub state: String,
}

#[derive(Debug, Clone)]
pub struct AnimationSnapshot {
    pub fragment_id: u32,
    pub tag: String,
    pub channels: Vec<AnimationChannelSnapshot>,
}

fn format_color(c: &Color) -> String {
    let rgba = c.to_rgba8();
    if rgba.a == 255 {
        format!("#{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b)
    } else {
        format!("#{:02x}{:02x}{:02x}{:02x}", rgba.r, rgba.g, rgba.b, rgba.a)
    }
}

fn serialize_taffy_style(props: &mut HashMap<String, String>, style: &taffy::Style) {
    match style.flex_direction {
        taffy::FlexDirection::Row => {}
        taffy::FlexDirection::Column => { props.insert("flexDirection".into(), "column".into()); }
        taffy::FlexDirection::RowReverse => { props.insert("flexDirection".into(), "row-reverse".into()); }
        taffy::FlexDirection::ColumnReverse => { props.insert("flexDirection".into(), "column-reverse".into()); }
    }
    if style.flex_grow != 0.0 {
        props.insert("flexGrow".into(), format!("{}", style.flex_grow));
    }
    if style.flex_shrink != 0.0 {
        props.insert("flexShrink".into(), format!("{}", style.flex_shrink));
    }
    if let Some(v) = lp_length_value(style.gap.width) {
        if v != 0.0 { props.insert("columnGap".into(), format!("{}", v)); }
    }
    if let Some(v) = lp_length_value(style.gap.height) {
        if v != 0.0 { props.insert("rowGap".into(), format!("{}", v)); }
    }
    serialize_taffy_rect_lp("padding", &style.padding, props);
    if let Some(ai) = style.align_items {
        props.insert("alignItems".into(), format!("{:?}", ai).to_ascii_lowercase());
    }
    if let Some(jc) = style.justify_content {
        props.insert("justifyContent".into(), format!("{:?}", jc).to_ascii_lowercase());
    }
    if style.overflow.x != taffy::Overflow::Visible || style.overflow.y != taffy::Overflow::Visible {
        props.insert("overflow".into(), format!("{:?}/{:?}", style.overflow.x, style.overflow.y).to_ascii_lowercase());
    }
}

fn lp_length_value(lp: taffy::LengthPercentage) -> Option<f32> {
    use taffy::style::CompactLength;
    let raw = lp.into_raw();
    if raw.tag() == CompactLength::LENGTH_TAG {
        Some(raw.value())
    } else {
        None
    }
}

fn serialize_taffy_rect_lp(
    prefix: &str,
    rect: &taffy::geometry::Rect<taffy::LengthPercentage>,
    props: &mut HashMap<String, String>,
) {
    let sides = [("Top", rect.top), ("Right", rect.right), ("Bottom", rect.bottom), ("Left", rect.left)];
    for (suffix, val) in sides {
        if let Some(v) = lp_length_value(val) {
            if v != 0.0 {
                props.insert(format!("{}{}", prefix, suffix), format!("{}", v));
            }
        }
    }
}

fn snapshot_tag(kind: &FragmentData) -> &'static str {
    match kind {
        FragmentData::Group(_) => "group",
        FragmentData::Rect(_) => "rect",
        FragmentData::Circle(_) => "circle",
        FragmentData::Path(_) => "path",
        FragmentData::Text(_) => "text",
        FragmentData::TextInput(_) => "textinput",
        FragmentData::Image(_) => "image",
        FragmentData::Span(_) => "span",
    }
}

impl FragmentTree {
    /// Devtools snapshot — returns a flat list of all nodes with layout data.
    pub fn snapshot(&self) -> Vec<FragmentNodeSnapshot> {
        self.nodes.values().map(|node| {
            let tag = snapshot_tag(&node.kind).to_string();

            let width = node.layout.width;
            let height = node.layout.height;

            let taffy_style = node.taffy_node
                .and_then(|tn| self.taffy.style(tn).ok())
                .cloned();

            let mut props = HashMap::new();
            match &node.kind {
                FragmentData::Rect(r) => {
                    if let Some(FragmentBrush::Solid(fp)) = &r.fill {
                        props.insert("fill".into(), format_color(&fp.color));
                    }
                    let radii = &r.corner_radii;
                    if radii.as_single_radius().map_or(true, |r| r > 0.0) {
                        props.insert("cornerRadius".into(), format!("{:.1}", radii.as_single_radius().unwrap_or(0.0)));
                    }
                    if r.stroke_width > 0.0 {
                        props.insert("strokeWidth".into(), format!("{:.1}", r.stroke_width));
                    }
                    if let Some(sp) = &r.stroke {
                        props.insert("stroke".into(), format_color(&sp.color));
                    }
                }
                FragmentData::Text(t) => {
                    props.insert("text".into(), t.text.clone());
                    props.insert("fontSize".into(), format!("{}", t.font_size));
                    if !t.font_family.is_empty() {
                        props.insert("fontFamily".into(), t.font_family.clone());
                    }
                    props.insert("color".into(), format_color(&t.color));
                }
                FragmentData::TextInput(t) => {
                    props.insert("text".into(), t.text.clone());
                    props.insert("fontSize".into(), format!("{}", t.font_size));
                    props.insert("color".into(), format_color(&t.color));
                }
                FragmentData::Circle(c) => {
                    props.insert("cx".into(), format!("{}", c.cx));
                    props.insert("cy".into(), format!("{}", c.cy));
                    props.insert("r".into(), format!("{}", c.r));
                    if let Some(fp) = &c.fill {
                        props.insert("fill".into(), format_color(&fp.color));
                    }
                }
                FragmentData::Path(p) => {
                    if !p.d.is_empty() {
                        props.insert("d".into(), p.d.clone());
                    }
                    if let Some(FragmentBrush::Solid(fp)) = &p.fill {
                        props.insert("fill".into(), format_color(&fp.color));
                    }
                }
                FragmentData::Group(_) => {}
                FragmentData::Image(img) => {
                    if !img.object_fit.is_empty() {
                        props.insert("objectFit".into(), img.object_fit.clone());
                    }
                    props.insert("hasImage".into(), img.image_data.is_some().to_string());
                }
                FragmentData::Span(s) => {
                    props.insert("text".into(), s.text.clone());
                    props.insert("fontSize".into(), format!("{}", s.font_size));
                    if !s.font_family.is_empty() {
                        props.insert("fontFamily".into(), s.font_family.clone());
                    }
                    props.insert("color".into(), format_color(&s.color));
                }
            }

            if let Some(style) = &taffy_style {
                serialize_taffy_style(&mut props, style);
            }

            FragmentNodeSnapshot {
                id: node.id.0,
                tag,
                parent_id: node.parent.map(|p| p.0),
                child_ids: node.children.iter().map(|c| c.0).collect(),
                x: node.render_x(),
                y: node.render_y(),
                width,
                height,
                clip: node.props.clip,
                visible: node.props.visible,
                opacity: node.props.opacity,
                props,
            }
        }).collect()
    }

    /// Snapshot of promoted layers for devtools LayerTree domain.
    pub fn snapshot_layers(&mut self) -> Vec<LayerSnapshot> {
        self.ensure_aabbs();
        self.nodes.values()
            .filter(|n| n.promoted && n.layer_key.is_some())
            .map(|n| {
                let bounds = n.world_aabb.unwrap_or(Rect::ZERO);
                let reasons = if n.props.opacity < 1.0 - f32::EPSILON && n.props.clip {
                    "opacity,clip"
                } else if n.props.opacity < 1.0 - f32::EPSILON {
                    "opacity"
                } else if n.props.clip {
                    "clip"
                } else {
                    "explicitly promoted"
                };
                LayerSnapshot {
                    fragment_id: n.id.0,
                    layer_key: n.layer_key.unwrap().0,
                    x: bounds.x0,
                    y: bounds.y0,
                    width: bounds.width(),
                    height: bounds.height(),
                    opacity: n.props.opacity,
                    reasons: reasons.to_string(),
                }
            })
            .collect()
    }

    /// Snapshot of active animations for devtools Animation domain.
    pub fn snapshot_animations(&self) -> Vec<AnimationSnapshot> {
        self.nodes.values()
            .filter_map(|n| {
                let timeline = n.timeline.as_ref()?;
                if !timeline.is_animating() { return None; }
                let channels = timeline.running_channel_snapshots()
                    .into_iter()
                    .map(|(prop, origin, target, state)| AnimationChannelSnapshot {
                        property: prop.to_string(),
                        origin,
                        target,
                        state: state.to_string(),
                    })
                    .collect();
                Some(AnimationSnapshot {
                    fragment_id: n.id.0,
                    tag: snapshot_tag(&n.kind).to_string(),
                    channels,
                })
            })
            .collect()
    }
}

pub fn fragment_store_snapshot(canvas_node_id: u32) -> Vec<FragmentNodeSnapshot> {
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.snapshot())
        .unwrap_or_default()
}

pub fn fragment_store_snapshot_layers(canvas_node_id: u32) -> Vec<LayerSnapshot> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.snapshot_layers())
        .unwrap_or_default()
}

pub fn fragment_store_snapshot_animations(canvas_node_id: u32) -> Vec<AnimationSnapshot> {
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.snapshot_animations())
        .unwrap_or_default()
}
