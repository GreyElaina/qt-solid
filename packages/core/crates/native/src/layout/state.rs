//! Rust-owned layout state for child widgets.

use std::collections::HashMap;
use std::sync::{LazyLock, Mutex};

use super::TaffyEngine;

pub const NO_TAFFY_HANDLE: u32 = u32::MAX;

#[derive(Debug, Clone)]
pub struct ChildLayout {
    pub flex_grow: i32,
    pub flex_shrink: i32,
    pub flex_basis: i32,
    pub max_width: i32,
    pub max_height: i32,
    pub margin: i32,
    pub align_self_tag: u8,
    pub aspect_ratio: f32,
    pub taffy_child_handle: u32,
    pub taffy_engine_id: u32,
    pub min_width: i32,
    pub min_height: i32,
}

impl Default for ChildLayout {
    fn default() -> Self {
        Self {
            flex_grow: 0,
            flex_shrink: 0,
            flex_basis: -1,
            max_width: -1,
            max_height: -1,
            margin: 0,
            align_self_tag: 1,
            aspect_ratio: 0.0,
            taffy_child_handle: NO_TAFFY_HANDLE,
            taffy_engine_id: 0,
            min_width: 0,
            min_height: 0,
        }
    }
}

/// Apply all child layout properties to the taffy engine.
pub fn replay_child_taffy_style(layout: &ChildLayout, engine: &mut TaffyEngine) -> bool {
    if layout.taffy_child_handle == NO_TAFFY_HANDLE {
        return false;
    }
    let n = layout.taffy_child_handle;
    engine.set_flex_grow(n, layout.flex_grow as f32);
    engine.set_flex_shrink(n, layout.flex_shrink as f32);
    if layout.flex_basis >= 0 {
        engine.set_flex_basis_px(n, layout.flex_basis as f32);
    } else {
        engine.set_flex_basis_auto(n);
    }
    engine.set_min_width_px(n, layout.min_width as f32);
    engine.set_min_height_px(n, layout.min_height as f32);
    if layout.max_width >= 0 {
        engine.set_max_width_px(n, layout.max_width as f32);
    }
    if layout.max_height >= 0 {
        engine.set_max_height_px(n, layout.max_height as f32);
    }
    engine.set_align_self(n, layout.align_self_tag);
    let m = layout.margin as f32;
    engine.set_margin_px(n, m, m, m, m);
    engine.set_aspect_ratio(n, layout.aspect_ratio);
    true
}

/// Per-engine metadata.
struct EngineEntry {
    engine: TaffyEngine,
}

/// Engines + per-widget child layout.
/// Single lock, Qt is single-threaded so contention is zero.
pub struct LayoutRegistry {
    engines: HashMap<u32, EngineEntry>,
    next_engine_id: u32,
    child_layouts: HashMap<u32, ChildLayout>,
}

impl LayoutRegistry {
    fn new() -> Self {
        Self {
            engines: HashMap::new(),
            next_engine_id: 1,
            child_layouts: HashMap::new(),
        }
    }

    // --- Engine lifecycle ---

    pub fn create_engine(&mut self) -> (u32, u32) {
        let id = self.next_engine_id;
        self.next_engine_id += 1;
        let mut engine = TaffyEngine::new();
        let root = engine.create_node();
        self.engines.insert(id, EngineEntry { engine });
        (id, root)
    }

    pub fn destroy_engine(&mut self, engine_id: u32) {
        self.engines.remove(&engine_id);
    }

    pub fn engine(&self, engine_id: u32) -> &TaffyEngine {
        &self
            .engines
            .get(&engine_id)
            .expect("invalid engine id")
            .engine
    }

    pub fn engine_mut(&mut self, engine_id: u32) -> &mut TaffyEngine {
        &mut self
            .engines
            .get_mut(&engine_id)
            .expect("invalid engine id")
            .engine
    }

    // --- Child layout ---

    pub fn register_child(&mut self, widget_id: u32) {
        self.child_layouts.entry(widget_id).or_default();
    }

    pub fn unregister_child(&mut self, widget_id: u32) {
        self.child_layouts.remove(&widget_id);
    }

    pub fn child_layout_mut(&mut self, widget_id: u32) -> &mut ChildLayout {
        self.child_layouts
            .get_mut(&widget_id)
            .expect("widget not in child layout registry")
    }
}

// SAFETY: LayoutRegistry is only accessed from Qt's main thread.
// TaffyTree contains *const () (CompactLength) which is !Send, but we
// guarantee single-threaded access. The Mutex is only for interior mutability.
unsafe impl Send for LayoutRegistry {}

static LAYOUT: LazyLock<Mutex<LayoutRegistry>> =
    LazyLock::new(|| Mutex::new(LayoutRegistry::new()));

pub fn with_layout<R>(f: impl FnOnce(&mut LayoutRegistry) -> R) -> R {
    f(&mut LAYOUT.lock().unwrap())
}
