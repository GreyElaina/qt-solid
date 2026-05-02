use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;

static WINDOW_A11Y: Lazy<Mutex<HashMap<u32, WindowA11yState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ── macOS ──────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use std::ffi::c_void;

    use accesskit::{ActionData, ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};
    use accesskit_macos::SubclassingAdapter;

    use crate::canvas::fragment::accessibility::text::{
        char_index_to_utf16_offset, node_id_to_fragment_id,
    };
    use crate::canvas::fragment::FragmentData;

    unsafe extern "C" {
        fn qt_solid_bridge_nswindow_accessibility(nsview_ptr: *mut c_void) -> bool;
    }

    struct FragmentActivationHandler {
        canvas_node_id: u32,
    }

    impl ActivationHandler for FragmentActivationHandler {
        fn request_initial_tree(&mut self) -> Option<TreeUpdate> {
            crate::runtime::with_fragment_tree(self.canvas_node_id, |tree| {
                tree.build_full_accesskit_update()
            })
        }
    }

    struct FragmentActionHandler {
        canvas_node_id: u32,
    }

    impl ActionHandler for FragmentActionHandler {
        fn do_action(&mut self, request: ActionRequest) {
            let canvas_id = self.canvas_node_id;

            let Some(frag_id) = node_id_to_fragment_id(request.target_node) else {
                return;
            };

            match request.action {
                accesskit::Action::Focus => {
                    let (old, new) =
                        crate::canvas::fragment::fragment_store_focus_fragment(canvas_id, frag_id);
                    if old != new {
                        crate::runtime::emit_js_event(crate::api::QtHostEvent::CanvasFocusChange {
                            canvas_node_id: canvas_id,
                            old_fragment_id: old,
                            new_fragment_id: new,
                        });
                    }
                    crate::qt::ffi::sync_text_edit_session_for_focus(canvas_id);
                }

                accesskit::Action::Click => {
                    // Default action: focus the node (same as click for most elements).
                    let (old, new) =
                        crate::canvas::fragment::fragment_store_focus_fragment(canvas_id, frag_id);
                    if old != new {
                        crate::runtime::emit_js_event(crate::api::QtHostEvent::CanvasFocusChange {
                            canvas_node_id: canvas_id,
                            old_fragment_id: old,
                            new_fragment_id: new,
                        });
                    }
                    crate::qt::ffi::sync_text_edit_session_for_focus(canvas_id);
                }

                accesskit::Action::SetTextSelection => {
                    if let Some(ActionData::SetTextSelection(sel)) = request.data {
                        dispatch_set_text_selection(canvas_id, frag_id, &sel);
                    }
                }

                _ => {}
            }
        }
    }

    fn dispatch_set_text_selection(
        canvas_id: u32,
        frag_id: crate::canvas::fragment::FragmentId,
        sel: &accesskit::TextSelection,
    ) {
        let result = crate::runtime::with_fragment_tree_mut(canvas_id, |tree| {
            let node = tree.nodes.get_mut(&frag_id)?;
            let ti = match &mut node.kind {
                FragmentData::TextInput(ti) => ti,
                _ => return None,
            };

            let cursor_utf16 = char_index_to_utf16_offset(&ti.text, sel.focus.character_index);
            let anchor_utf16 = char_index_to_utf16_offset(&ti.text, sel.anchor.character_index);

            ti.cursor_pos = cursor_utf16 as f64;
            ti.selection_anchor = if cursor_utf16 == anchor_utf16 {
                -1.0
            } else {
                anchor_utf16 as f64
            };

            node.dirty = true;
            tree.semantics_dirty.insert(frag_id);

            let (sel_start, sel_end) = if ti.selection_anchor >= 0.0 {
                let c = ti.cursor_pos as i32;
                let a = ti.selection_anchor as i32;
                (c.min(a), c.max(a))
            } else {
                (-1, -1)
            };

            Some((
                frag_id.0,
                ti.text.clone(),
                ti.cursor_pos as i32,
                sel_start,
                sel_end,
            ))
        })
        .flatten();

        if let Some((fragment_id, text, cursor, sel_start, sel_end)) = result {
            crate::runtime::emit_js_event(crate::api::QtHostEvent::CanvasTextInputChange {
                canvas_node_id: canvas_id,
                fragment_id,
                text,
                cursor,
                sel_start,
                sel_end,
            });
        }
    }

    pub(super) struct PlatformA11yState {
        adapter: SendAdapter,
        canvas_node_id: u32,
    }

    struct SendAdapter(SubclassingAdapter);
    unsafe impl Send for SendAdapter {}

    pub(super) fn create(window_node_id: u32, nsview_ptr: u64) -> PlatformA11yState {
        let adapter = unsafe {
            SubclassingAdapter::new(
                nsview_ptr as *mut c_void,
                FragmentActivationHandler {
                    canvas_node_id: window_node_id,
                },
                FragmentActionHandler { canvas_node_id: window_node_id },
            )
        };

        unsafe { qt_solid_bridge_nswindow_accessibility(nsview_ptr as *mut c_void) };

        PlatformA11yState {
            adapter: SendAdapter(adapter),
            canvas_node_id: window_node_id,
        }
    }

    pub(super) fn update_tree(state: &mut PlatformA11yState) {
        let canvas_id = state.canvas_node_id;
        if let Some(events) = state.adapter.0.update_if_active(|| {
            crate::runtime::with_fragment_tree(canvas_id, |tree| {
                tree.build_full_accesskit_update()
            })
            .unwrap_or_else(|| {
                // Empty tree fallback
                accesskit::TreeUpdate {
                    nodes: vec![],
                    tree: None,
                    tree_id: accesskit::TreeId::ROOT,
                    focus: accesskit::NodeId(u64::MAX),
                }
            })
        }) {
            events.raise();
        }
    }

    pub(super) fn update_focus(state: &mut PlatformA11yState, focused: bool) {
        if let Some(events) = state.adapter.0.update_view_focus_state(focused) {
            events.raise();
        }
    }
}

// ── non-macOS stubs ────────────────────────────────────────────────────

#[cfg(not(target_os = "macos"))]
mod platform {
    pub(super) struct PlatformA11yState;

    pub(super) fn create(_window_node_id: u32, _handle: u64) -> PlatformA11yState {
        PlatformA11yState
    }

    pub(super) fn update_tree(_state: &mut PlatformA11yState) {}

    pub(super) fn update_focus(_state: &mut PlatformA11yState, _focused: bool) {}
}

// ── public API ─────────────────────────────────────────────────────────

struct WindowA11yState {
    platform: platform::PlatformA11yState,
}

pub(crate) fn init_window_accessibility(window_node_id: u32, native_handle: u64) {
    let mut map = WINDOW_A11Y.lock().expect("a11y mutex poisoned");
    if map.contains_key(&window_node_id) {
        return;
    }
    let state = WindowA11yState {
        platform: platform::create(window_node_id, native_handle),
    };
    map.insert(window_node_id, state);
}

/// Push the current fragment tree state to the accessibility adapter.
/// Called after each successful frame present.
pub(crate) fn update_window_accessibility_tree(window_node_id: u32) {
    let mut map = WINDOW_A11Y.lock().expect("a11y mutex poisoned");
    if let Some(state) = map.get_mut(&window_node_id) {
        platform::update_tree(&mut state.platform);
    }
}

pub(crate) fn update_window_accessibility(window_node_id: u32, focused: bool) {
    let mut map = WINDOW_A11Y.lock().expect("a11y mutex poisoned");
    if let Some(state) = map.get_mut(&window_node_id) {
        platform::update_focus(&mut state.platform, focused);
    }
}

pub(crate) fn destroy_window_accessibility(window_node_id: u32) {
    let mut map = WINDOW_A11Y.lock().expect("a11y mutex poisoned");
    map.remove(&window_node_id);
}
