use std::collections::HashSet;

use crate::canvas::fragment::paint::transform_local_bounds_to_world;
use crate::canvas::fragment::types::FragmentId;
use crate::vello::peniko::kurbo::{Rect, Shape};
use crate::vello::Scene;

use super::FragmentTree;
use anyrender::recording::RenderCommand;

/// Maximum number of independent dirty rects before falling back to union.
const MAX_DIRTY_RECTS: usize = 8;

/// Result of dirty rect computation.
#[derive(Debug)]
pub enum DirtyRectResult {
    /// Full repaint required (first frame, resize, clear_all).
    FullRepaint,
    /// Nothing is dirty — no render needed (noop blit).
    NothingDirty,
    /// Partial update — list of dirty rects in device pixels (x, y, w, h).
    Partial(Vec<(u32, u32, u32, u32)>),
}

impl FragmentTree {
    /// Compute dirty rects from individual dirty node bounds.
    ///
    /// Returns a `DirtyRectResult` indicating full repaint, nothing dirty,
    /// or a list of partial device-pixel rects.
    pub fn compute_dirty_rects(&mut self, scale_factor: f64) -> DirtyRectResult {
        if self.force_full_repaint {
            return DirtyRectResult::FullRepaint;
        }
        if self.dirty_node_ids.is_empty() && self.stale_node_bounds.is_empty() {
            return DirtyRectResult::NothingDirty;
        }

        // Ensure world AABBs are up to date for new bounds estimation.
        self.ensure_aabbs();

        // Collect per-node dirty rects: union of old bounds and new bounds.
        let all_ids: HashSet<FragmentId> = self
            .stale_node_bounds
            .keys()
            .copied()
            .chain(self.dirty_node_ids.iter().copied())
            .collect();

        let mut rects: Vec<Rect> = Vec::with_capacity(all_ids.len());
        for id in all_ids {
            let mut node_rect: Option<Rect> = None;
            if let Some(&stale) = self.stale_node_bounds.get(&id) {
                node_rect = Some(stale);
            }
            if let Some(node) = self.nodes.get(&id) {
                if let Some(aabb) = node.world_aabb {
                    node_rect = Some(match node_rect {
                        Some(r) => r.union(aabb),
                        None => aabb,
                    });
                }
            }
            if let Some(r) = node_rect {
                rects.push(r);
            }
        }

        if rects.is_empty() {
            return DirtyRectResult::NothingDirty;
        }

        // Convert to device pixels first — tile math operates in pixel space.
        let mut device_rects: Vec<(u32, u32, u32, u32)> = rects
            .iter()
            .map(|r| {
                let x = (r.x0 * scale_factor).floor().max(0.0) as u32;
                let y = (r.y0 * scale_factor).floor().max(0.0) as u32;
                let x1 = (r.x1 * scale_factor).ceil() as u32;
                let y1 = (r.y1 * scale_factor).ceil() as u32;
                (x, y, x1.saturating_sub(x), y1.saturating_sub(y))
            })
            .collect();

        // Tile-aware merge: use wide tile cost (256×4) instead of pixel area.
        merge_dirty_rects_tile_aware(&mut device_rects);

        DirtyRectResult::Partial(device_rects)
    }

    /// Clear stale bounds and force_full_repaint after a frame has been presented.
    pub fn consume_dirty_state(&mut self) {
        self.stale_node_bounds.clear();
        self.dirty_node_ids.clear();
        self.force_full_repaint = false;
    }

}

// ---------------------------------------------------------------------------
// Scene bounds computation from anyrender RenderCommands
// ---------------------------------------------------------------------------

/// Compute the axis-aligned bounding box of all render commands in a scene.
pub(crate) fn scene_bounds(scene: &Scene) -> Option<Rect> {
    let mut result: Option<Rect> = None;

    for cmd in &scene.commands {
        let cmd_bounds = match cmd {
            RenderCommand::Fill(fill) => {
                let local = fill.shape.bounding_box();
                Some(transform_local_bounds_to_world(local, fill.transform))
            }
            RenderCommand::Stroke(stroke) => {
                let local = stroke.shape.bounding_box();
                let inflated = inflate_rect(local, stroke.style.width / 2.0);
                Some(transform_local_bounds_to_world(inflated, stroke.transform))
            }
            RenderCommand::BoxShadow(bs) => {
                let extent = bs.std_dev * 3.0 + bs.radius;
                let inflated = inflate_rect(bs.rect, extent);
                Some(transform_local_bounds_to_world(inflated, bs.transform))
            }
            RenderCommand::GlyphRun(gr) => {
                // Conservative estimate: x range from glyphs, height from font_size.
                if gr.glyphs.is_empty() {
                    None
                } else {
                    let fs = gr.font_size as f64;
                    let min_x = gr.glyphs.iter().map(|g| g.x as f64).fold(f64::INFINITY, f64::min);
                    let max_x = gr.glyphs.iter().map(|g| g.x as f64).fold(f64::NEG_INFINITY, f64::max);
                    // Last glyph advance approximated as 0.6 * font_size.
                    let local = Rect::new(min_x, -fs * 0.8, max_x + fs * 0.6, fs * 0.2);
                    Some(transform_local_bounds_to_world(local, gr.transform))
                }
            }
            RenderCommand::PushLayer(_) | RenderCommand::PushClipLayer(_) | RenderCommand::PopLayer => None,
        };

        if let Some(b) = cmd_bounds {
            result = Some(match result {
                Some(r) => r.union(b),
                None => b,
            });
        }
    }

    result
}

pub(crate) fn inflate_rect(r: Rect, amount: f64) -> Rect {
    Rect::new(r.x0 - amount, r.y0 - amount, r.x1 + amount, r.y1 + amount)
}

pub(crate) fn rects_intersect(a: Rect, b: Rect) -> bool {
    a.x0 < b.x1 && a.x1 > b.x0 && a.y0 < b.y1 && a.y1 > b.y0
}

// ---------------------------------------------------------------------------
// Tile-aware dirty rect merging
// ---------------------------------------------------------------------------

/// Wide tile dimensions matching vello_hybrid's coarse rasterizer.
/// Content strips and clip regions are dispatched at this granularity.
const WIDE_TILE_W: u32 = 256;
const WIDE_TILE_H: u32 = 4;

/// Count how many wide tiles a device-pixel rect covers.
fn wide_tile_count(x: u32, y: u32, w: u32, h: u32) -> u32 {
    if w == 0 || h == 0 {
        return 0;
    }
    let tx0 = x / WIDE_TILE_W;
    let ty0 = y / WIDE_TILE_H;
    let tx1 = (x + w + WIDE_TILE_W - 1) / WIDE_TILE_W;
    let ty1 = (y + h + WIDE_TILE_H - 1) / WIDE_TILE_H;
    (tx1 - tx0) * (ty1 - ty0)
}

/// Union two device-pixel rects.
fn union_device_rect(
    a: (u32, u32, u32, u32),
    b: (u32, u32, u32, u32),
) -> (u32, u32, u32, u32) {
    let x0 = a.0.min(b.0);
    let y0 = a.1.min(b.1);
    let x1 = (a.0 + a.2).max(b.0 + b.2);
    let y1 = (a.1 + a.3).max(b.1 + b.3);
    (x0, y0, x1 - x0, y1 - y0)
}

/// Tile-aware dirty rect merge (O(n³) worst case, fine for n ≤ ~20).
///
/// Strategy:
/// 1. Unconditionally merge overlapping rects (tile_waste ≤ 0).
/// 2. Repeatedly merge the pair with the smallest tile_waste until
///    count ≤ MAX_DIRTY_RECTS.
/// 3. Opportunistically merge pairs where tile_waste = 0 even when
///    under the cap (same tile row, no extra tiles).
fn merge_dirty_rects_tile_aware(rects: &mut Vec<(u32, u32, u32, u32)>) {
    // Phase 1: merge all overlapping pairs (free or negative waste).
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < rects.len() {
            let mut j = i + 1;
            while j < rects.len() {
                let (ax, ay, aw, ah) = rects[i];
                let (bx, by, bw, bh) = rects[j];
                // Check pixel overlap.
                let overlaps = ax < bx + bw && ax + aw > bx && ay < by + bh && ay + ah > by;
                if overlaps {
                    rects[i] = union_device_rect(rects[i], rects[j]);
                    rects.swap_remove(j);
                    changed = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }

    // Phase 2: reduce to MAX_DIRTY_RECTS by merging the cheapest pair
    // (measured in wide tiles added).
    while rects.len() > MAX_DIRTY_RECTS {
        let (mi, mj) = cheapest_merge_pair(rects);
        rects[mi] = union_device_rect(rects[mi], rects[mj]);
        rects.swap_remove(mj);
    }

    // Phase 3: opportunistic zero-cost merges (same tile rows, no extra tiles).
    let mut changed = true;
    while changed {
        changed = false;
        let mut i = 0;
        while i < rects.len() {
            let mut j = i + 1;
            while j < rects.len() {
                let merged = union_device_rect(rects[i], rects[j]);
                let cost_separate =
                    wide_tile_count(rects[i].0, rects[i].1, rects[i].2, rects[i].3)
                        + wide_tile_count(rects[j].0, rects[j].1, rects[j].2, rects[j].3);
                let cost_merged = wide_tile_count(merged.0, merged.1, merged.2, merged.3);
                if cost_merged <= cost_separate {
                    rects[i] = merged;
                    rects.swap_remove(j);
                    changed = true;
                } else {
                    j += 1;
                }
            }
            i += 1;
        }
    }
}

/// Find the pair (i, j) whose merge adds the fewest extra wide tiles.
fn cheapest_merge_pair(rects: &[(u32, u32, u32, u32)]) -> (usize, usize) {
    let mut best = (0, 1);
    let mut best_waste = u32::MAX;
    for i in 0..rects.len() {
        let ti = wide_tile_count(rects[i].0, rects[i].1, rects[i].2, rects[i].3);
        for j in (i + 1)..rects.len() {
            let tj = wide_tile_count(rects[j].0, rects[j].1, rects[j].2, rects[j].3);
            let merged = union_device_rect(rects[i], rects[j]);
            let tm = wide_tile_count(merged.0, merged.1, merged.2, merged.3);
            let waste = tm.saturating_sub(ti + tj);
            if waste < best_waste {
                best_waste = waste;
                best = (i, j);
            }
        }
    }
    best
}
