use std::collections::HashMap;
use std::sync::Mutex;

use once_cell::sync::Lazy;

static WINDOW_A11Y: Lazy<Mutex<HashMap<u32, WindowA11yState>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

// ── macOS ──────────────────────────────────────────────────────────────

#[cfg(target_os = "macos")]
mod platform {
    use std::collections::HashMap;
    use std::ffi::c_void;
    use std::sync::Mutex;

    use accesskit::{ActionData, ActionHandler, ActionRequest, ActivationHandler, TreeUpdate};
    use accesskit_macos::Adapter;
    use objc2::ffi;
    use objc2::runtime::{AnyClass, AnyObject, Imp, Sel};
    use objc2::{msg_send, sel};
    use objc2_app_kit::NSView;
    use once_cell::sync::Lazy;

    use crate::canvas::fragment::accessibility::text::{
        char_index_to_utf16_offset, node_id_to_fragment_id,
    };
    use crate::canvas::fragment::FragmentData;

    use crate::accessibility_bridge;

    // ── View → adapter dispatch map ────────────────────────────────────

    struct AdapterEntry {
        adapter: Adapter,
        activation_handler: Box<dyn ActivationHandler>,
    }

    // SAFETY: Adapter is !Send but we only access it from the main thread
    // (ObjC accessibility callbacks are always main-thread). The Mutex is
    // only for interior mutability, not cross-thread use.
    unsafe impl Send for AdapterEntry {}

    /// Maps NSView pointer → AdapterEntry for accessibility IMP dispatch.
    static VIEW_ADAPTERS: Lazy<Mutex<HashMap<usize, AdapterEntry>>> =
        Lazy::new(|| Mutex::new(HashMap::new()));

    fn with_adapter<R>(
        view_ptr: usize,
        f: impl FnOnce(&mut Adapter, &mut dyn ActivationHandler) -> R,
    ) -> Option<R> {
        let mut map = VIEW_ADAPTERS.lock().ok()?;
        let entry = map.get_mut(&view_ptr)?;
        Some(f(&mut entry.adapter, &mut *entry.activation_handler))
    }

    // ── Accessibility IMPs ─────────────────────────────────────────────
    //
    // These return `*mut AnyObject` (raw ObjC id) to avoid Rust type
    // mismatches between our objc2 0.6 and accesskit's objc2 0.5.
    // At the ObjC ABI level the types are identical.

    unsafe extern "C" fn a11y_children(this: &NSView, _cmd: Sel) -> *mut AnyObject {
        let ptr = this as *const NSView as usize;
        with_adapter(ptr, |adapter, handler| adapter.view_children(handler) as *mut AnyObject)
            .unwrap_or(std::ptr::null_mut())
    }

    unsafe extern "C" fn a11y_focused_element(this: &NSView, _cmd: Sel) -> *mut AnyObject {
        let ptr = this as *const NSView as usize;
        with_adapter(ptr, |adapter, handler| adapter.focus(handler) as *mut AnyObject)
            .unwrap_or(std::ptr::null_mut())
    }

    #[repr(C)]
    #[derive(Clone, Copy)]
    struct CGPoint {
        x: f64,
        y: f64,
    }

    unsafe extern "C" fn a11y_hit_test(this: &NSView, _cmd: Sel, point: CGPoint) -> *mut AnyObject {
        let ptr = this as *const NSView as usize;
        // Transmute our CGPoint to accesskit's NSPoint — identical C layout (two f64s).
        with_adapter(ptr, |adapter, handler| {
            let ns_point = unsafe { std::mem::transmute::<CGPoint, _>(point) };
            adapter.hit_test(ns_point, handler) as *mut AnyObject
        })
            .unwrap_or(std::ptr::null_mut())
    }

    // ── Patch accessibility methods onto QNSView class ─────────────────

    static VIEW_CLASS_PATCHED: Mutex<bool> = Mutex::new(false);

    /// Replaces accessibilityChildren, accessibilityFocusedUIElement, and
    /// accessibilityHitTest: on the QNSView *declared* class (obtained via
    /// `[view class]`). Uses `class_replaceMethod` so it works whether or
    /// not QNSView already implements these (Qt does implement some).
    ///
    /// This is safe w.r.t. KVO because it modifies the declared class, not
    /// any `NSKVONotifying_*` isa-swizzled subclass.
    unsafe fn patch_view_class(view: &NSView) {
        let mut patched = VIEW_CLASS_PATCHED.lock().unwrap();
        if *patched {
            return;
        }

        // SAFETY: We are swizzling accessibility methods on the concrete view
        // class. This is safe because we only do it once (guarded by `patched`)
        // and we target the declared class, not a KVO subclass.
        unsafe {
            let cls: *const AnyClass = msg_send![view, class];
            let cls = cls as *mut AnyClass;

            ffi::class_replaceMethod(
                cls,
                sel!(accessibilityChildren),
                std::mem::transmute::<
                    unsafe extern "C" fn(&NSView, Sel) -> *mut AnyObject,
                    Imp,
                >(a11y_children),
                c"@@:".as_ptr(),
            );

            ffi::class_replaceMethod(
                cls,
                sel!(accessibilityFocusedUIElement),
                std::mem::transmute::<
                    unsafe extern "C" fn(&NSView, Sel) -> *mut AnyObject,
                    Imp,
                >(a11y_focused_element),
                c"@@:".as_ptr(),
            );

            ffi::class_replaceMethod(
                cls,
                sel!(accessibilityHitTest:),
                std::mem::transmute::<
                    unsafe extern "C" fn(&NSView, Sel, CGPoint) -> *mut AnyObject,
                    Imp,
                >(a11y_hit_test),
                c"@@:{CGPoint=dd}".as_ptr(),
            );
        }

        *patched = true;
    }

    // ── Activation / action handlers ───────────────────────────────────

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

    // ── Platform state ─────────────────────────────────────────────────

    pub(super) struct PlatformA11yState {
        view_ptr: usize,
        canvas_node_id: u32,
    }

    pub(super) fn create(window_node_id: u32, nsview_ptr: u64) -> PlatformA11yState {
        let view = unsafe { &*(nsview_ptr as *const NSView) };

        // Patch accessibility methods onto QNSView class (no isa-swizzle).
        unsafe { patch_view_class(view) };

        // Also patch NSWindow accessibilityChildren bridge.
        unsafe { accessibility_bridge::bridge_nswindow_accessibility(nsview_ptr as *mut c_void) };

        // Create adapter (no isa-swizzle, just tree management).
        let adapter = unsafe {
            Adapter::new(
                nsview_ptr as *mut c_void,
                false,
                FragmentActionHandler { canvas_node_id: window_node_id },
            )
        };

        let activation_handler = Box::new(FragmentActivationHandler {
            canvas_node_id: window_node_id,
        });

        let view_key = nsview_ptr as usize;
        VIEW_ADAPTERS.lock().unwrap().insert(view_key, AdapterEntry {
            adapter,
            activation_handler,
        });

        PlatformA11yState {
            view_ptr: view_key,
            canvas_node_id: window_node_id,
        }
    }

    pub(super) fn update_tree(state: &mut PlatformA11yState) {
        let canvas_id = state.canvas_node_id;
        let mut map = VIEW_ADAPTERS.lock().unwrap();
        let Some(entry) = map.get_mut(&state.view_ptr) else {
            return;
        };
        if let Some(events) = entry.adapter.update_if_active(|| {
            crate::runtime::with_fragment_tree(canvas_id, |tree| {
                tree.build_full_accesskit_update()
            })
            .unwrap_or_else(|| {
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
        let mut map = VIEW_ADAPTERS.lock().unwrap();
        let Some(entry) = map.get_mut(&state.view_ptr) else {
            return;
        };
        if let Some(events) = entry.adapter.update_view_focus_state(focused) {
            events.raise();
        }
    }

    pub(super) fn destroy(state: &PlatformA11yState) {
        VIEW_ADAPTERS.lock().unwrap().remove(&state.view_ptr);
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

    pub(super) fn destroy(_state: &PlatformA11yState) {}
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
    if let Some(state) = map.remove(&window_node_id) {
        platform::destroy(&state.platform);
    }
}
