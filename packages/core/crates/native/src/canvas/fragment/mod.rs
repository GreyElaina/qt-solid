pub(crate) mod accessibility;
pub mod decl;
mod encode;
mod hit_test;
mod kinds;
pub mod layout;
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

use super::vello::Scene;
use super::vello::peniko::kurbo::{Affine, BezPath, Point, Rect, Vec2};
use super::vello::peniko::{Color, ImageData};
use crate::renderer::compositor::effects::{BackdropBlurEffect, InnerShadowEffect};
use crate::runtime;
use decl::FragmentValue;
use layout::{
    AbsoluteInset, CellAlign, Container, CrossAlign, Direction, EdgeInsets, Overflow, Placement,
    PrimaryAlign, SizeIntent, Sizing, TrackSize, WrapDistribute,
};

pub fn fragment_store_ensure(canvas_node_id: u32) {
    runtime::ensure_fragment_tree(canvas_node_id);
}

pub fn fragment_store_remove(canvas_node_id: u32) {
    runtime::remove_fragment_tree(canvas_node_id);
}

pub fn fragment_store_create_node(canvas_node_id: u32, tag: &str) -> Option<FragmentId> {
    // "grid" is a virtual tag — creates a RectFragment with Container::Grid pre-set.
    let is_grid = tag == "grid";
    let effective_tag = if is_grid { "rect" } else { tag };
    let kind = FragmentData::from_tag_loose(effective_tag)?;
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        let id = tree.create_node(kind);
        if is_grid {
            if let Some(node) = tree.nodes.get_mut(&id) {
                node.container = Some(Container::Grid {
                    columns: Vec::new(),
                    rows: Vec::new(),
                    column_gap: 0.0,
                    row_gap: 0.0,
                });
                node.layout_dirty = true;
            }
        }
        id
    })
}

pub fn fragment_store_insert_child(
    canvas_node_id: u32,
    parent: Option<FragmentId>,
    child: FragmentId,
    before: Option<FragmentId>,
) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.insert_child(parent, child, before);

        // Child's taffy style depends on parent direction — re-derive after reparenting.
        if let Some(node) = tree.nodes.get_mut(&child) {
            node.layout_dirty = true;
        }
        tree.any_dirty = true;

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
        let is_span = tree
            .nodes
            .get(&child)
            .map_or(false, |n| matches!(n.kind, FragmentData::Span(_)));
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
            }
        }
        tree.invalidate_subtree_cache_for(fragment_id);
    });
}

pub fn fragment_store_clear_image_data(canvas_node_id: u32, fragment_id: FragmentId) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            if let FragmentData::Image(ref mut img) = node.kind {
                img.image_data = None;
                node.dirty = true;
                tree.any_dirty = true;
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
        // Semantics props — accessibility data (intercept before layout/visual paths)
        if is_semantics_prop(key) {
            if let Some(node) = tree.nodes.get_mut(&fragment_id) {
                apply_semantics_prop(node, key, &value);
                tree.semantics_dirty.insert(fragment_id);
            }
            return;
        }

        // Layout intent props — all layout keys (Figma-style + legacy CSS-style)
        if is_layout_intent_prop(key) {
            if let Some(node) = tree.nodes.get_mut(&fragment_id) {
                apply_layout_intent_prop(node, key, &value);
                node.layout_dirty = true;
            }
            tree.any_dirty = true;
            tree.aabbs_dirty = true;
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

        // Mask child — needs tree-level access for two-node mutation.
        if key == "maskChild" {
            match &value {
                FragmentValue::F64 { value: v } => {
                    let mask_fid = FragmentId(*v as u32);
                    if let Some(node) = tree.nodes.get_mut(&fragment_id) {
                        node.mask_child = Some(mask_fid);
                    }
                    if let Some(mask_node) = tree.nodes.get_mut(&mask_fid) {
                        mask_node.is_mask_source = true;
                        mask_node.promoted = true;
                    }
                    // Pull mask child out of flex flow so it doesn't shift
                    // sibling layout. Absolute positioning with no inset
                    // places it at (0,0) relative to the parent.
                    tree.with_taffy_style_mut(mask_fid, |style| {
                        style.position = taffy::style::Position::Absolute;
                    });
                }
                FragmentValue::Unset => {
                    let old_mask = tree.nodes.get_mut(&fragment_id)
                        .and_then(|n| n.mask_child.take());
                    if let Some(old_fid) = old_mask {
                        if let Some(mask_node) = tree.nodes.get_mut(&old_fid) {
                            mask_node.is_mask_source = false;
                            mask_node.promoted = false;
                        }
                        tree.with_taffy_style_mut(old_fid, |style| {
                            style.position = taffy::style::Position::Relative;
                        });
                    }
                }
                _ => {}
            }
            tree.any_dirty = true;
            tree.invalidate_subtree_cache_for(fragment_id);
            return;
        }

        // Track explicit width/height → sizing intent + explicit paint size
        if key == "width" || key == "height" {
            if let FragmentValue::F64 { value: v } = &value {
                let fv = *v;
                if let Some(node) = tree.nodes.get_mut(&fragment_id) {
                    if key == "width" {
                        node.props.explicit_width = if fv > 0.0 { Some(fv) } else { None };
                        placement_sizing_mut(&mut node.placement).w = if fv > 0.0 {
                            Sizing::Fixed(fv as f32)
                        } else {
                            Sizing::Hug
                        };
                    } else {
                        node.props.explicit_height = if fv > 0.0 { Some(fv) } else { None };
                        placement_sizing_mut(&mut node.placement).h = if fv > 0.0 {
                            Sizing::Fixed(fv as f32)
                        } else {
                            Sizing::Hug
                        };
                    }
                    node.layout_dirty = true;
                }
            } else if let FragmentValue::Str { ref value } = value {
                if let Some(sizing) = parse_dimension_string_to_sizing(value) {
                    if let Some(node) = tree.nodes.get_mut(&fragment_id) {
                        if key == "width" {
                            node.props.explicit_width = None;
                            placement_sizing_mut(&mut node.placement).w = sizing;
                        } else {
                            node.props.explicit_height = None;
                            placement_sizing_mut(&mut node.placement).h = sizing;
                        }
                        node.layout_dirty = true;
                    }
                    tree.any_dirty = true;
                }
            }
        }

        if let Some(node) = tree.nodes.get_mut(&fragment_id) {
            apply_fragment_prop(node, key, value);
            if let FragmentData::TextInput(ref mut input) = node.kind {
                input.ensure_caret_visible();
            }
            node.dirty = true;
            tree.any_dirty = true;
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

        // Sync placement when x/y explicit state changes.
        if key == "x" || key == "y" {
            if let Some(node) = tree.nodes.get_mut(&fragment_id) {
                let ex = node.props.explicit_x;
                let ey = node.props.explicit_y;
                if ex.is_some() || ey.is_some() {
                    let sizing = match &node.placement {
                        Placement::Flow { sizing, .. } => *sizing,
                        Placement::Absolute { sizing, .. } => *sizing,
                        Placement::GridCell { sizing, .. } => *sizing,
                    };
                    node.placement = Placement::Absolute {
                        inset: AbsoluteInset {
                            left: ex.map(|v| v as f32),
                            top: ey.map(|v| v as f32),
                            right: None,
                            bottom: None,
                        },
                        sizing,
                    };
                } else if matches!(node.placement, Placement::Absolute { .. }) {
                    let sizing = match &node.placement {
                        Placement::Absolute { sizing, .. } => *sizing,
                        _ => unreachable!(),
                    };
                    node.placement = Placement::Flow {
                        sizing,
                        align_self: None,
                        margin: None,
                    };
                }
                node.layout_dirty = true;
            }
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
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.paint_subtrees())
        .unwrap_or_default()
}

pub fn fragment_store_compute_dirty_rects(
    canvas_node_id: u32,
    scale_factor: f64,
) -> DirtyRectResult {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.compute_dirty_rects(scale_factor)
    })
    .unwrap_or(DirtyRectResult::FullRepaint)
}

pub fn fragment_store_consume_dirty_state(canvas_node_id: u32) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.consume_dirty_state();
    });
}

pub fn fragment_store_force_full_repaint(canvas_node_id: u32) -> bool {
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.force_full_repaint).unwrap_or(true)
}

pub fn fragment_store_request_full_repaint(canvas_node_id: u32) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.request_full_repaint();
    });
}

pub fn fragment_store_paint_single(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    scene: &mut Scene,
    transform: Affine,
) {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.paint_node_self_only(scene, fragment_id, transform);
    });
}

pub fn fragment_store_paint_at_origin(
    canvas_node_id: u32,
    fragment_id: FragmentId,
    scene: &mut Scene,
    transform: Affine,
) {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.paint_node_at_origin(scene, fragment_id, transform);
    });
}

pub fn fragment_store_world_bounds(canvas_node_id: u32, fragment_id: FragmentId) -> Option<Rect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.ensure_aabbs();
        tree.node(fragment_id)?.world_aabb
    })
    .flatten()
}

pub fn fragment_store_build_paint_plan(canvas_node_id: u32) -> Option<PaintPlan> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.build_paint_plan())
}

pub fn fragment_store_build_render_plan(canvas_node_id: u32) -> Option<RenderPlan> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.build_paint_plan().partition())
}

pub fn fragment_store_has_promoted(canvas_node_id: u32) -> bool {
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.has_promoted_nodes()).unwrap_or(false)
}

pub fn fragment_store_collect_inner_shadows(
    canvas_node_id: u32,
    scale_factor: f64,
) -> Vec<InnerShadowEffect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.collect_inner_shadow_effects(scale_factor)
    })
    .unwrap_or_default()
}

pub fn fragment_store_collect_backdrop_blurs(
    canvas_node_id: u32,
    scale_factor: f64,
) -> Vec<BackdropBlurEffect> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.collect_backdrop_blur_effects(scale_factor)
    })
    .unwrap_or_default()
}

pub fn fragment_store_has_animating(canvas_node_id: u32) -> bool {
    runtime::with_fragment_tree(canvas_node_id, |tree| {
        tree.nodes
            .values()
            .any(|n| n.timeline.as_ref().map_or(false, |t| t.is_animating()))
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
        (old.map(|id| id.0 as i32).unwrap_or(-1), new_focused)
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
                ti.ensure_caret_visible();
            }
            node.dirty = true;
        }
        tree.any_dirty = true;
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
                ti.ensure_caret_visible();
            }
            node.dirty = true;
        }
        tree.any_dirty = true;
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
            Some((
                text.text.clone(),
                text.font_size,
                text.font_family.clone(),
                weight,
                italic,
                text.text_max_width,
                text.text_overflow.clone(),
            ))
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
                    font_size: if span.font_size > 0.0 {
                        span.font_size
                    } else {
                        text_frag.font_size
                    },
                    font_family: if span.font_family.is_empty() {
                        text_frag.font_family.clone()
                    } else {
                        span.font_family.clone()
                    },
                    font_weight: if span.font_weight > 0.0 {
                        span.font_weight as i32
                    } else {
                        text_frag.font_weight as i32
                    },
                    font_italic: if span.font_style.is_empty() {
                        text_frag.font_style == "italic"
                    } else {
                        span.font_style == "italic"
                    },
                    color: span.color,
                });
            }
        }
        if runs.is_empty() {
            None
        } else {
            Some((
                runs,
                text_frag.font_size,
                text_frag.font_family.clone(),
                text_frag.text_max_width,
                text_frag.text_overflow.clone(),
            ))
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
    })
    .flatten()
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
            Some((
                ti.text.clone(),
                ti.font_size,
                ti.font_family.clone(),
                weight,
                italic,
            ))
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
    let text_x = runtime::with_fragment_tree(canvas_node_id, |tree| {
        let target = resolve_text_input(tree, fragment_id)?;
        let world = tree.world_transform(target);
        let inv = world.inverse();
        let local = inv * Point::new(window_x, 0.0);
        let node = tree.node(target)?;
        if let FragmentData::TextInput(ref input) = node.kind {
            Some(input.visible_x_to_text_x(local.x))
        } else {
            Some(local.x)
        }
    })
    .flatten();

    if let Some(x) = text_x {
        let _ = crate::qt::ffi::qt_text_edit_click_to_cursor(canvas_node_id, x);
    }
}

pub fn fragment_store_drag_to_cursor(canvas_node_id: u32, window_x: f64, _window_y: f64) {
    let text_x = runtime::with_fragment_tree(canvas_node_id, |tree| {
        let focused_id = tree.focused()?;
        let target = resolve_text_input(tree, focused_id)?;
        let world = tree.world_transform(target);
        let inv = world.inverse();
        let local = inv * Point::new(window_x, 0.0);
        let node = tree.node(target)?;
        if let FragmentData::TextInput(ref input) = node.kind {
            Some(input.visible_x_to_text_x(local.x))
        } else {
            Some(local.x)
        }
    })
    .flatten();

    if let Some(x) = text_x {
        let _ = crate::qt::ffi::qt_text_edit_drag_to_cursor(canvas_node_id, x);
    }
}

/// If `id` is a TextInput, return it; otherwise DFS-search its children
/// for the first TextInput descendant.
fn resolve_text_input(tree: &FragmentTree, id: FragmentId) -> Option<FragmentId> {
    let node = tree.node(id)?;
    if matches!(node.kind, FragmentData::TextInput(_)) {
        return Some(id);
    }
    find_first_text_input_child(tree, &node.children)
}

fn find_first_text_input_child(tree: &FragmentTree, children: &[FragmentId]) -> Option<FragmentId> {
    for &child_id in children {
        let child = tree.node(child_id)?;
        if matches!(child.kind, FragmentData::TextInput(_)) {
            return Some(child_id);
        }
        if let Some(found) = find_first_text_input_child(tree, &child.children) {
            return Some(found);
        }
    }
    None
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
    let now = crate::qt::trace_now_ns() as f64 / 1_000_000_000.0;
    let events = runtime::with_fragment_tree_mut(canvas_node_id, |tree| {
        tree.frame_now = now;
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
        tree.set_motion_target(
            fragment_id,
            targets,
            default_transition,
            per_property,
            delay_secs,
            now,
        )
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
        tree.set_motion_target_keyframes(
            fragment_id,
            targets,
            times,
            default_transition,
            per_property,
            delay_secs,
            now,
        )
    })
    .unwrap_or(false)
}

pub fn fragment_store_tick_motion(canvas_node_id: u32, now: f64) -> (bool, Vec<FragmentId>, f64) {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.tick_motion(now)).unwrap_or((
        false,
        Vec::new(),
        0.0,
    ))
}

pub fn fragment_store_get_world_bounds(
    canvas_node_id: u32,
    fragment_id: FragmentId,
) -> Option<Rect> {
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
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.get_content_size(fragment_id)).flatten()
}

pub fn fragment_store_compute_intrinsic_size(canvas_node_id: u32) -> Option<(f64, f64)> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.compute_intrinsic_size()).flatten()
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
// Layout intent prop keys
// ---------------------------------------------------------------------------

const LAYOUT_INTENT_PROPS: &[&str] = &[
    "w",
    "h",
    "direction",
    "primaryAlign",
    "crossAlign",
    "crossGap",
    "wrapDistribute",
    "layoutVisible",
    "layoutOverflow",
    "layoutOverflowX",
    "layoutOverflowY",
    "layoutPadding",
    "layoutPaddingTop",
    "layoutPaddingRight",
    "layoutPaddingBottom",
    "layoutPaddingLeft",
    "layoutPosition",
    "layoutTop",
    "layoutRight",
    "layoutBottom",
    "layoutLeft",
    "layoutAlignSelf",
    "layoutMargin",
    "layoutMarginTop",
    "layoutMarginRight",
    "layoutMarginBottom",
    "layoutMarginLeft",
    "layoutMinWidth",
    "layoutMinHeight",
    "layoutMaxWidth",
    "layoutMaxHeight",
    "layoutGridColumns",
    "layoutGridRows",
    "layoutGridColumnGap",
    "layoutGridRowGap",
    "layoutGridRow",
    "layoutGridColumn",
    "layoutGridRowSpan",
    "layoutGridColSpan",
    "layoutGridHAlign",
    "layoutGridVAlign",
    "gap",
    "wrap",
    "padding",
    "paddingTop",
    "paddingRight",
    "paddingBottom",
    "paddingLeft",
];

fn is_layout_intent_prop(key: &str) -> bool {
    LAYOUT_INTENT_PROPS.contains(&key)
}

fn parse_sizing(value: &FragmentValue) -> Option<Sizing> {
    match value {
        FragmentValue::Str { value } => match value.as_str() {
            "hug" => Some(Sizing::Hug),
            "fill" => Some(Sizing::Fill),
            s => {
                if let Some(fr) = s.strip_suffix("fr") {
                    fr.parse::<f32>().ok().map(Sizing::Flex)
                } else if let Some(pct) = s.strip_suffix('%') {
                    pct.parse::<f32>().ok().map(|v| Sizing::Percent(v / 100.0))
                } else {
                    None
                }
            }
        },
        FragmentValue::F64 { value } => Some(Sizing::Fixed(*value as f32)),
        _ => None,
    }
}

fn placement_sizing_mut(placement: &mut Placement) -> &mut SizeIntent {
    match placement {
        Placement::Flow { sizing, .. } => sizing,
        Placement::Absolute { sizing, .. } => sizing,
        Placement::GridCell { sizing, .. } => sizing,
    }
}

fn apply_layout_intent_prop(node: &mut FragmentNode, key: &str, value: &FragmentValue) {
    match key {
        // ─── Sizing ───
        "w" => {
            if let Some(s) = parse_sizing(value) {
                placement_sizing_mut(&mut node.placement).w = s;
            }
        }
        "h" => {
            if let Some(s) = parse_sizing(value) {
                placement_sizing_mut(&mut node.placement).h = s;
            }
        }
        "layoutMinWidth" => {
            if let FragmentValue::F64 { value } = value {
                placement_sizing_mut(&mut node.placement).min_w = Some(*value as f32);
            }
        }
        "layoutMinHeight" => {
            if let FragmentValue::F64 { value } = value {
                placement_sizing_mut(&mut node.placement).min_h = Some(*value as f32);
            }
        }
        "layoutMaxWidth" => {
            if let FragmentValue::F64 { value } = value {
                placement_sizing_mut(&mut node.placement).max_w = Some(*value as f32);
            }
        }
        "layoutMaxHeight" => {
            if let FragmentValue::F64 { value } = value {
                placement_sizing_mut(&mut node.placement).max_h = Some(*value as f32);
            }
        }

        // ─── Container (Flex) ───
        "direction" => {
            if let FragmentValue::Str { value } = value {
                let dir = match value.as_str() {
                    "horizontal" => Direction::Horizontal,
                    _ => Direction::Vertical,
                };
                match &mut node.container {
                    Some(Container::Flex { direction, .. }) => *direction = dir,
                    _ => {
                        node.container = Some(Container::Flex {
                            direction: dir,
                            primary_align: PrimaryAlign::default(),
                            cross_align: CrossAlign::default(),
                            gap: 0.0,
                            cross_gap: None,
                            wrap: false,
                            wrap_distribute: WrapDistribute::default(),
                        });
                    }
                }
            }
        }
        "primaryAlign" => {
            if let FragmentValue::Str { value } = value {
                let align = match value.as_str() {
                    "center" => PrimaryAlign::Center,
                    "end" => PrimaryAlign::End,
                    "space-between" => PrimaryAlign::SpaceBetween,
                    "space-around" => PrimaryAlign::SpaceAround,
                    "space-evenly" => PrimaryAlign::SpaceEvenly,
                    _ => PrimaryAlign::Start,
                };
                ensure_flex_container(node);
                if let Some(Container::Flex { primary_align, .. }) = &mut node.container {
                    *primary_align = align;
                }
            }
        }
        "crossAlign" => {
            if let FragmentValue::Str { value } = value {
                let align = match value.as_str() {
                    "center" => CrossAlign::Center,
                    "end" => CrossAlign::End,
                    "stretch" => CrossAlign::Stretch,
                    "baseline" => CrossAlign::Baseline,
                    _ => CrossAlign::Start,
                };
                ensure_flex_container(node);
                if let Some(Container::Flex { cross_align, .. }) = &mut node.container {
                    *cross_align = align;
                }
            }
        }
        "gap" => {
            if let FragmentValue::F64 { value } = value {
                ensure_flex_container(node);
                if let Some(Container::Flex { gap, .. }) = &mut node.container {
                    *gap = *value as f32;
                }
            }
        }
        "crossGap" => {
            if let FragmentValue::F64 { value } = value {
                ensure_flex_container(node);
                if let Some(Container::Flex { cross_gap, .. }) = &mut node.container {
                    *cross_gap = Some(*value as f32);
                }
            }
        }
        "wrap" => {
            if let FragmentValue::Bool { value } = value {
                ensure_flex_container(node);
                if let Some(Container::Flex { wrap, .. }) = &mut node.container {
                    *wrap = *value;
                }
            }
        }
        "wrapDistribute" => {
            if let FragmentValue::Str { value } = value {
                let dist = match value.as_str() {
                    "space-between" => WrapDistribute::SpaceBetween,
                    _ => WrapDistribute::Packed,
                };
                ensure_flex_container(node);
                if let Some(Container::Flex { wrap_distribute, .. }) = &mut node.container {
                    *wrap_distribute = dist;
                }
            }
        }

        // ─── Padding ───
        "layoutPadding" => {
            if let FragmentValue::F64 { value } = value {
                let v = *value as f32;
                node.padding = EdgeInsets { top: v, right: v, bottom: v, left: v };
            }
        }
        "layoutPaddingTop" => {
            if let FragmentValue::F64 { value } = value {
                node.padding.top = *value as f32;
            }
        }
        "layoutPaddingRight" => {
            if let FragmentValue::F64 { value } = value {
                node.padding.right = *value as f32;
            }
        }
        "layoutPaddingBottom" => {
            if let FragmentValue::F64 { value } = value {
                node.padding.bottom = *value as f32;
            }
        }
        "layoutPaddingLeft" => {
            if let FragmentValue::F64 { value } = value {
                node.padding.left = *value as f32;
            }
        }

        // ─── Overflow / Visibility ───
        "layoutOverflow" => {
            if let FragmentValue::Str { value } = value {
                let ov = match value.as_str() {
                    "clip" => Overflow::Clip,
                    "hidden" => Overflow::Hidden,
                    "scroll" => Overflow::Scroll,
                    _ => Overflow::Visible,
                };
                node.overflow_x = ov;
                node.overflow_y = ov;
            }
        }
        "layoutOverflowX" => {
            if let FragmentValue::Str { value } = value {
                node.overflow_x = match value.as_str() {
                    "clip" => Overflow::Clip,
                    "hidden" => Overflow::Hidden,
                    "scroll" => Overflow::Scroll,
                    _ => Overflow::Visible,
                };
            }
        }
        "layoutOverflowY" => {
            if let FragmentValue::Str { value } = value {
                node.overflow_y = match value.as_str() {
                    "clip" => Overflow::Clip,
                    "hidden" => Overflow::Hidden,
                    "scroll" => Overflow::Scroll,
                    _ => Overflow::Visible,
                };
            }
        }
        "layoutVisible" => {
            if let FragmentValue::Bool { value } = value {
                node.layout_visible = *value;
            }
        }

        // ─── Placement: Absolute ───
        "layoutPosition" => {
            if let FragmentValue::Str { value } = value {
                if value == "absolute" {
                    let sizing = match &node.placement {
                        Placement::Flow { sizing, .. } => *sizing,
                        Placement::Absolute { sizing, .. } => *sizing,
                        Placement::GridCell { sizing, .. } => *sizing,
                    };
                    node.placement = Placement::Absolute {
                        inset: AbsoluteInset::default(),
                        sizing,
                    };
                } else {
                    // Back to flow
                    let sizing = match &node.placement {
                        Placement::Flow { sizing, .. } => *sizing,
                        Placement::Absolute { sizing, .. } => *sizing,
                        Placement::GridCell { sizing, .. } => *sizing,
                    };
                    node.placement = Placement::Flow {
                        sizing,
                        align_self: None,
                        margin: None,
                    };
                }
            }
        }
        "layoutTop" => {
            if let FragmentValue::F64 { value } = value {
                if let Placement::Absolute { inset, .. } = &mut node.placement {
                    inset.top = Some(*value as f32);
                }
            }
        }
        "layoutRight" => {
            if let FragmentValue::F64 { value } = value {
                if let Placement::Absolute { inset, .. } = &mut node.placement {
                    inset.right = Some(*value as f32);
                }
            }
        }
        "layoutBottom" => {
            if let FragmentValue::F64 { value } = value {
                if let Placement::Absolute { inset, .. } = &mut node.placement {
                    inset.bottom = Some(*value as f32);
                }
            }
        }
        "layoutLeft" => {
            if let FragmentValue::F64 { value } = value {
                if let Placement::Absolute { inset, .. } = &mut node.placement {
                    inset.left = Some(*value as f32);
                }
            }
        }

        // ─── Placement: align_self / margin ───
        "layoutAlignSelf" => {
            if let FragmentValue::Str { value } = value {
                let a = match value.as_str() {
                    "center" => Some(CrossAlign::Center),
                    "end" => Some(CrossAlign::End),
                    "stretch" => Some(CrossAlign::Stretch),
                    "baseline" => Some(CrossAlign::Baseline),
                    "start" => Some(CrossAlign::Start),
                    _ => None,
                };
                if let Placement::Flow { align_self, .. } = &mut node.placement {
                    *align_self = a;
                }
            }
        }
        "layoutMargin" => {
            if let FragmentValue::F64 { value } = value {
                let v = *value as f32;
                let m = EdgeInsets { top: v, right: v, bottom: v, left: v };
                if let Placement::Flow { margin, .. } = &mut node.placement {
                    *margin = Some(m);
                }
            }
        }
        "layoutMarginTop" => {
            if let FragmentValue::F64 { value } = value {
                if let Placement::Flow { margin, .. } = &mut node.placement {
                    margin.get_or_insert(EdgeInsets::default()).top = *value as f32;
                }
            }
        }
        "layoutMarginRight" => {
            if let FragmentValue::F64 { value } = value {
                if let Placement::Flow { margin, .. } = &mut node.placement {
                    margin.get_or_insert(EdgeInsets::default()).right = *value as f32;
                }
            }
        }
        "layoutMarginBottom" => {
            if let FragmentValue::F64 { value } = value {
                if let Placement::Flow { margin, .. } = &mut node.placement {
                    margin.get_or_insert(EdgeInsets::default()).bottom = *value as f32;
                }
            }
        }
        "layoutMarginLeft" => {
            if let FragmentValue::F64 { value } = value {
                if let Placement::Flow { margin, .. } = &mut node.placement {
                    margin.get_or_insert(EdgeInsets::default()).left = *value as f32;
                }
            }
        }

        // ─── Grid container ───
        "layoutGridColumns" => {
            if let FragmentValue::GridTracks { tracks } = value {
                let cols: Vec<TrackSize> = tracks.iter().map(|t| parse_intent_track(t)).collect();
                match &mut node.container {
                    Some(Container::Grid { columns, .. }) => *columns = cols,
                    _ => {
                        node.container = Some(Container::Grid {
                            columns: cols,
                            rows: Vec::new(),
                            column_gap: 0.0,
                            row_gap: 0.0,
                        });
                    }
                }
            }
        }
        "layoutGridRows" => {
            if let FragmentValue::GridTracks { tracks } = value {
                let rs: Vec<TrackSize> = tracks.iter().map(|t| parse_intent_track(t)).collect();
                match &mut node.container {
                    Some(Container::Grid { rows, .. }) => *rows = rs,
                    _ => {
                        node.container = Some(Container::Grid {
                            columns: Vec::new(),
                            rows: rs,
                            column_gap: 0.0,
                            row_gap: 0.0,
                        });
                    }
                }
            }
        }
        "layoutGridColumnGap" => {
            if let FragmentValue::F64 { value } = value {
                if let Some(Container::Grid { column_gap, .. }) = &mut node.container {
                    *column_gap = *value as f32;
                }
            }
        }
        "layoutGridRowGap" => {
            if let FragmentValue::F64 { value } = value {
                if let Some(Container::Grid { row_gap, .. }) = &mut node.container {
                    *row_gap = *value as f32;
                }
            }
        }

        // ─── Grid child ───
        "layoutGridRow" => {
            if let FragmentValue::F64 { value } = value {
                ensure_grid_cell(node);
                if let Placement::GridCell { row, .. } = &mut node.placement {
                    *row = *value as u16;
                }
            }
        }
        "layoutGridColumn" => {
            if let FragmentValue::F64 { value } = value {
                ensure_grid_cell(node);
                if let Placement::GridCell { column, .. } = &mut node.placement {
                    *column = *value as u16;
                }
            }
        }
        "layoutGridRowSpan" => {
            if let FragmentValue::F64 { value } = value {
                ensure_grid_cell(node);
                if let Placement::GridCell { row_span, .. } = &mut node.placement {
                    *row_span = (*value as u16).max(1);
                }
            }
        }
        "layoutGridColSpan" => {
            if let FragmentValue::F64 { value } = value {
                ensure_grid_cell(node);
                if let Placement::GridCell { col_span, .. } = &mut node.placement {
                    *col_span = (*value as u16).max(1);
                }
            }
        }
        "layoutGridHAlign" => {
            if let FragmentValue::Str { value } = value {
                let a = match value.as_str() {
                    "start" => CellAlign::Start,
                    "center" => CellAlign::Center,
                    "end" => CellAlign::End,
                    _ => CellAlign::Auto,
                };
                if let Placement::GridCell { h_align, .. } = &mut node.placement {
                    *h_align = a;
                }
            }
        }
        "layoutGridVAlign" => {
            if let FragmentValue::Str { value } = value {
                let a = match value.as_str() {
                    "start" => CellAlign::Start,
                    "center" => CellAlign::Center,
                    "end" => CellAlign::End,
                    _ => CellAlign::Auto,
                };
                if let Placement::GridCell { v_align, .. } = &mut node.placement {
                    *v_align = a;
                }
            }
        }

        _ => {}
    }
}

fn ensure_flex_container(node: &mut FragmentNode) {
    if !matches!(node.container, Some(Container::Flex { .. })) {
        node.container = Some(Container::Flex {
            direction: Direction::default(),
            primary_align: PrimaryAlign::default(),
            cross_align: CrossAlign::default(),
            gap: 0.0,
            cross_gap: None,
            wrap: false,
            wrap_distribute: WrapDistribute::default(),
        });
    }
}

fn ensure_grid_cell(node: &mut FragmentNode) {
    if !matches!(node.placement, Placement::GridCell { .. }) {
        let sizing = match &node.placement {
            Placement::Flow { sizing, .. } => *sizing,
            Placement::Absolute { sizing, .. } => *sizing,
            Placement::GridCell { sizing, .. } => *sizing,
        };
        node.placement = Placement::GridCell {
            row: 0,
            column: 0,
            row_span: 1,
            col_span: 1,
            h_align: CellAlign::Auto,
            v_align: CellAlign::Auto,
            sizing,
        };
    }
}

fn parse_intent_track(s: &str) -> TrackSize {
    let s = s.trim();
    if s == "hug" {
        return TrackSize::Hug;
    }
    if let Some(fr_str) = s.strip_suffix("fr") {
        if let Ok(v) = fr_str.trim().parse::<f32>() {
            return TrackSize::Flex(v);
        }
    }
    if let Ok(v) = s.parse::<f32>() {
        return TrackSize::Fixed(v);
    }
    TrackSize::Hug
}

fn is_semantics_prop(key: &str) -> bool {
    matches!(
        key,
        "role"
            | "ariaLabel"
            | "ariaDescription"
            | "ariaLive"
            | "ariaChecked"
            | "ariaExpanded"
            | "ariaSelected"
            | "ariaDisabled"
            | "ariaValueNow"
    )
}

fn apply_semantics_prop(node: &mut FragmentNode, key: &str, value: &FragmentValue) {
    let semantics = node
        .semantics
        .get_or_insert_with(|| SemanticsData::with_role(accesskit::Role::Group));

    match key {
        "role" => {
            if let FragmentValue::Str { value } = value {
                semantics.role = parse_a11y_role(value);
            }
        }
        "ariaLabel" => {
            if let FragmentValue::Str { value } = value {
                semantics.label = if value.is_empty() {
                    None
                } else {
                    Some(value.clone())
                };
            }
        }
        "ariaDescription" => {
            if let FragmentValue::Str { value } = value {
                semantics.description = if value.is_empty() {
                    None
                } else {
                    Some(value.clone())
                };
            }
        }
        "ariaLive" => {
            if let FragmentValue::Str { value } = value {
                semantics.live = match value.as_str() {
                    "polite" => Some(accesskit::Live::Polite),
                    "assertive" => Some(accesskit::Live::Assertive),
                    _ => None,
                };
            }
        }
        "ariaChecked" => {
            if let FragmentValue::Bool { value } = value {
                semantics.checked = Some(if *value {
                    accesskit::Toggled::True
                } else {
                    accesskit::Toggled::False
                });
            }
        }
        "ariaExpanded" => {
            if let FragmentValue::Bool { value } = value {
                semantics.expanded = Some(*value);
            }
        }
        "ariaSelected" => {
            if let FragmentValue::Bool { value } = value {
                semantics.selected = Some(*value);
            }
        }
        "ariaDisabled" => {
            if let FragmentValue::Bool { value } = value {
                semantics.disabled = *value;
            }
        }
        "ariaValueNow" => {
            if let FragmentValue::Str { value } = value {
                semantics.value = if value.is_empty() {
                    None
                } else {
                    Some(value.clone())
                };
            } else if let FragmentValue::F64 { value } = value {
                semantics.value = Some(value.to_string());
            }
        }
        _ => {}
    }
}

fn parse_a11y_role(role: &str) -> accesskit::Role {
    match role {
        "button" => accesskit::Role::Button,
        "checkbox" => accesskit::Role::CheckBox,
        "radio" => accesskit::Role::RadioButton,
        "switch" => accesskit::Role::Switch,
        "slider" => accesskit::Role::Slider,
        "spinbutton" => accesskit::Role::SpinButton,
        "textbox" => accesskit::Role::TextInput,
        "searchbox" => accesskit::Role::SearchInput,
        "combobox" => accesskit::Role::ComboBox,
        "list" => accesskit::Role::List,
        "listitem" => accesskit::Role::ListItem,
        "listbox" => accesskit::Role::ListBox,
        "option" => accesskit::Role::ListBoxOption,
        "menu" => accesskit::Role::Menu,
        "menuitem" => accesskit::Role::MenuItem,
        "menubar" => accesskit::Role::MenuBar,
        "tab" => accesskit::Role::Tab,
        "tablist" => accesskit::Role::TabList,
        "tabpanel" => accesskit::Role::TabPanel,
        "tree" => accesskit::Role::Tree,
        "treeitem" => accesskit::Role::TreeItem,
        "table" => accesskit::Role::Table,
        "row" => accesskit::Role::Row,
        "cell" => accesskit::Role::Cell,
        "link" => accesskit::Role::Link,
        "heading" => accesskit::Role::Heading,
        "img" | "image" => accesskit::Role::Image,
        "navigation" => accesskit::Role::Navigation,
        "main" => accesskit::Role::Main,
        "region" => accesskit::Role::Region,
        "form" => accesskit::Role::Form,
        "group" => accesskit::Role::Group,
        "dialog" => accesskit::Role::Dialog,
        "alert" => accesskit::Role::Alert,
        "alertdialog" => accesskit::Role::AlertDialog,
        "progressbar" => accesskit::Role::ProgressIndicator,
        "separator" => accesskit::Role::Splitter,
        "toolbar" => accesskit::Role::Toolbar,
        "tooltip" => accesskit::Role::Tooltip,
        "scrollbar" => accesskit::Role::ScrollBar,
        "label" => accesskit::Role::Label,
        _ => accesskit::Role::Group,
    }
}

/// Parse dimension strings like "100%", "50%", "auto" to Sizing.
fn parse_dimension_string_to_sizing(s: &str) -> Option<Sizing> {
    let s = s.trim();
    if s == "auto" {
        return Some(Sizing::Hug);
    }
    if let Some(pct) = s.strip_suffix('%') {
        if let Ok(v) = pct.trim().parse::<f32>() {
            return Some(Sizing::Percent(v / 100.0));
        }
    }
    None
}



fn apply_fragment_prop(node: &mut FragmentNode, key: &str, value: FragmentValue) {
    match key {
        "x" => {
            match value {
                FragmentValue::F64 { value } => {
                    node.props.explicit_x = Some(value);
                }
                FragmentValue::Unset => {
                    node.props.explicit_x = None;
                }
                _ => {}
            }
            return;
        }
        "y" => {
            match value {
                FragmentValue::F64 { value } => {
                    node.props.explicit_y = Some(value);
                }
                FragmentValue::Unset => {
                    node.props.explicit_y = None;
                }
                _ => {}
            }
            return;
        }
        "opacity" => {
            if let FragmentValue::F64 { value } = value {
                node.props.opacity = value as f32;
            }
            return;
        }
        "clip" => {
            if let FragmentValue::Bool { value } = value {
                node.props.clip = value;
            }
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
                FragmentValue::Unset => {
                    node.props.clip_path = None;
                }
                _ => {}
            }
            return;
        }
        "visible" => {
            if let FragmentValue::Bool { value } = value {
                node.props.visible = value;
            }
            return;
        }
        "pointerEvents" => {
            if let FragmentValue::Bool { value } = value {
                node.props.pointer_events = value;
            }
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
            if let FragmentValue::Bool { value } = value {
                node.props.focusable = value;
            }
            return;
        }
        "layer" => {
            if let FragmentValue::Bool { value } = value {
                node.promoted = value;
            }
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
                FragmentValue::Unset => {
                    node.props.backdrop_blur = None;
                }
                _ => {}
            }
            return;
        }
        "perspective" => {
            match value {
                FragmentValue::F64 { value } => {
                    node.perspective_pose.2 = value;
                }
                FragmentValue::Unset => {
                    node.perspective_pose.2 = 0.0;
                }
                _ => {}
            }
            return;
        }
        "filterGrayscale" | "filterSaturate" | "filterBrightness" | "filterContrast"
        | "filterHueRotate" | "filterInvert" | "filterSepia" => {
            match value {
                FragmentValue::F64 { value } => {
                    let filter = node.props.content_filter.get_or_insert_with(ContentFilterParams::default);
                    let v = value as f32;
                    match key {
                        "filterGrayscale" => filter.grayscale = v,
                        "filterSaturate" => filter.saturate = v,
                        "filterBrightness" => filter.brightness = v,
                        "filterContrast" => filter.contrast = v,
                        "filterHueRotate" => filter.hue_rotate = v,
                        "filterInvert" => filter.invert = v,
                        "filterSepia" => filter.sepia = v,
                        _ => {}
                    }
                    if filter.is_identity() {
                        node.props.content_filter = None;
                    }
                }
                FragmentValue::Unset => {
                    node.props.content_filter = None;
                }
                _ => {}
            }
            return;
        }
        "vibrancyDesaturation" => {
            match value {
                FragmentValue::F64 { value } => {
                    let v = node.props.vibrancy.get_or_insert(VibrancyParams {
                        desaturation: 0.5, blend_mode: 3, tint: [1.0, 1.0, 1.0, 0.85],
                    });
                    v.desaturation = value as f32;
                }
                FragmentValue::Unset => {
                    node.props.vibrancy = None;
                }
                _ => {}
            }
            return;
        }
        "vibrancyBlendMode" => {
            match value {
                FragmentValue::F64 { value } => {
                    let v = node.props.vibrancy.get_or_insert(VibrancyParams {
                        desaturation: 0.5, blend_mode: 3, tint: [1.0, 1.0, 1.0, 0.85],
                    });
                    v.blend_mode = value as u32;
                }
                FragmentValue::Unset => {
                    node.props.vibrancy = None;
                }
                _ => {}
            }
            return;
        }
        "zIndex" => {
            match value {
                FragmentValue::F64 { value } => {
                    node.props.z_index = value as i32;
                }
                FragmentValue::Unset => {
                    node.props.z_index = 0;
                }
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
    if matches!(
        key,
        "text"
            | "fontSize"
            | "fontFamily"
            | "fontWeight"
            | "fontStyle"
            | "textMaxWidth"
            | "textOverflow"
    ) {
        match &mut node.kind {
            FragmentData::Text(t) => {
                t.shaped = None;
            }
            FragmentData::TextInput(ti) => {
                ti.layout = None;
            }
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
    pub delay_ms: f64,
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
        taffy::FlexDirection::Row => {
            props.insert("direction".into(), "horizontal".into());
        }
        taffy::FlexDirection::Column => {}
        taffy::FlexDirection::RowReverse => {
            props.insert("direction".into(), "horizontal-reverse".into());
        }
        taffy::FlexDirection::ColumnReverse => {
            props.insert("direction".into(), "vertical-reverse".into());
        }
    }
    if style.flex_grow != 0.0 {
        props.insert("flexGrow".into(), format!("{}", style.flex_grow));
    }
    if style.flex_shrink != 0.0 {
        props.insert("flexShrink".into(), format!("{}", style.flex_shrink));
    }
    if let Some(v) = lp_length_value(style.gap.width) {
        if v != 0.0 {
            props.insert("columnGap".into(), format!("{}", v));
        }
    }
    if let Some(v) = lp_length_value(style.gap.height) {
        if v != 0.0 {
            props.insert("rowGap".into(), format!("{}", v));
        }
    }
    serialize_taffy_rect_lp("padding", &style.padding, props);
    if let Some(ai) = style.align_items {
        props.insert(
            "crossAlign".into(),
            format!("{:?}", ai).to_ascii_lowercase(),
        );
    }
    if let Some(jc) = style.justify_content {
        props.insert(
            "primaryAlign".into(),
            format!("{:?}", jc).to_ascii_lowercase(),
        );
    }
    if style.overflow.x != taffy::Overflow::Visible || style.overflow.y != taffy::Overflow::Visible
    {
        props.insert(
            "overflow".into(),
            format!("{:?}/{:?}", style.overflow.x, style.overflow.y).to_ascii_lowercase(),
        );
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
    let sides = [
        ("Top", rect.top),
        ("Right", rect.right),
        ("Bottom", rect.bottom),
        ("Left", rect.left),
    ];
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
        self.nodes
            .values()
            .map(|node| {
                let tag = snapshot_tag(&node.kind).to_string();

                let width = node.layout.width;
                let height = node.layout.height;

                let taffy_style = node
                    .taffy_node
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
                            props.insert(
                                "cornerRadius".into(),
                                format!("{:.1}", radii.as_single_radius().unwrap_or(0.0)),
                            );
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
            })
            .collect()
    }

    /// Snapshot of promoted layers for devtools LayerTree domain.
    pub fn snapshot_layers(&mut self) -> Vec<LayerSnapshot> {
        self.ensure_aabbs();
        self.nodes
            .values()
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
        let now = crate::qt::trace_now_ns() as f64 / 1_000_000_000.0;
        self.nodes
            .values()
            .filter_map(|n| {
                let timeline = n.timeline.as_ref()?;
                if !timeline.is_animating() {
                    return None;
                }
                let channels = timeline
                    .running_channel_snapshots(now)
                    .into_iter()
                    .map(|(prop, origin, target, state, delay_secs)| AnimationChannelSnapshot {
                        property: prop.to_string(),
                        origin,
                        target,
                        state: state.to_string(),
                        delay_ms: delay_secs * 1000.0,
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
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.snapshot()).unwrap_or_default()
}

pub fn fragment_store_snapshot_layers(canvas_node_id: u32) -> Vec<LayerSnapshot> {
    runtime::with_fragment_tree_mut(canvas_node_id, |tree| tree.snapshot_layers())
        .unwrap_or_default()
}

pub fn fragment_store_snapshot_animations(canvas_node_id: u32) -> Vec<AnimationSnapshot> {
    runtime::with_fragment_tree(canvas_node_id, |tree| tree.snapshot_animations())
        .unwrap_or_default()
}
