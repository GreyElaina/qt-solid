pub mod capture;
pub mod ffi;
pub mod tree;
pub mod types;

use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex, Weak,
        atomic::{AtomicBool, Ordering},
    },
    thread::{self, ThreadId},
};

use napi::{
    Env, Error, Result, Status,
    bindgen_prelude::{Function, FunctionRef},
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode, UnknownReturnValue},
};
use once_cell::sync::Lazy;

use self::tree::NodeTree;
use self::types::NodeKind;

pub(crate) use self::ffi::*;

use crate::{
    api::{QtHostEvent, QtNode},
    qt::{self},
    renderer::scheduler,
    trace,
};

pub(crate) const ROOT_NODE_ID: u32 = 1;

type EventCallbackTsfn =
    ThreadsafeFunction<QtHostEvent, UnknownReturnValue, QtHostEvent, Status, false>;
type EventCallbackRef = FunctionRef<QtHostEvent, UnknownReturnValue>;

struct EventCallback {
    owner_thread: ThreadId,
    env_raw: usize,
    direct: EventCallbackRef,
    fallback: EventCallbackTsfn,
}

impl EventCallback {
    fn new(env: &Env, direct: EventCallbackRef, fallback: EventCallbackTsfn) -> Self {
        Self {
            owner_thread: thread::current().id(),
            env_raw: env.raw() as usize,
            direct,
            fallback,
        }
    }

    fn is_owner_thread(&self) -> bool {
        thread::current().id() == self.owner_thread
    }

    fn call(&self, event: QtHostEvent) -> Result<()> {
        if self.is_owner_thread() {
            let env = Env::from_raw(self.env_raw as napi::sys::napi_env);
            return env.run_in_scope(|| {
                let callback = self.direct.borrow_back(&env)?;
                callback.call(event).map(|_| ())
            });
        }

        let status = self
            .fallback
            .call(event, ThreadsafeFunctionCallMode::NonBlocking);
        if status == Status::Ok {
            Ok(())
        } else {
            Err(Error::new(
                status,
                "failed to dispatch Qt host event".to_owned(),
            ))
        }
    }

    fn dispatch_mode(&self) -> &'static str {
        if self.is_owner_thread() {
            "direct"
        } else {
            "tsfn"
        }
    }
}

pub(crate) struct QtNodeInner {
    pub(crate) id: u32,
    pub(crate) generation: u64,
    pub(crate) kind: NodeKind,
    destroyed: AtomicBool,
}

impl QtNodeInner {
    pub(crate) fn new(id: u32, generation: u64, kind: NodeKind) -> Self {
        Self {
            id,
            generation,
            kind,
            destroyed: AtomicBool::new(false),
        }
    }

    pub(crate) fn is_window(&self) -> bool {
        self.kind.is_window()
    }

    fn is_destroyed(&self) -> bool {
        self.destroyed.load(Ordering::SeqCst)
    }

    fn mark_destroyed(&self) {
        self.destroyed.store(true, Ordering::SeqCst);
    }

    fn mark_destroyed_once(&self) -> bool {
        self.destroyed.swap(true, Ordering::SeqCst)
    }
}

pub(crate) trait NodeHandle {
    fn inner(&self) -> &Arc<QtNodeInner>;
}

struct RuntimeState {
    generation_counter: u64,
    app_generation: Option<u64>,
    next_node_id: u32,
    tree: NodeTree,
    wrappers: HashMap<u32, Weak<QtNodeInner>>,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            generation_counter: 0,
            app_generation: None,
            next_node_id: ROOT_NODE_ID + 1,
            tree: NodeTree::with_root(ROOT_NODE_ID),
            wrappers: HashMap::new(),
        }
    }

    fn start_new_app(&mut self) -> u64 {
        self.generation_counter += 1;
        let generation = self.generation_counter;
        self.app_generation = Some(generation);
        self.next_node_id = ROOT_NODE_ID + 1;
        self.tree.reset_with_root(ROOT_NODE_ID);
        self.wrappers.clear();
        crate::renderer::with_renderer_mut(|r| r.clear_all());
        generation
    }

    fn invalidate_all(&mut self) {
        self.generation_counter += 1;

        for weak in self.wrappers.values() {
            if let Some(inner) = weak.upgrade() {
                inner.mark_destroyed();
            }
        }

        self.app_generation = None;
        self.next_node_id = ROOT_NODE_ID + 1;
        self.tree.reset_with_root(ROOT_NODE_ID);
        self.wrappers.clear();
        crate::renderer::with_renderer_mut(|r| r.clear_all());
    }

    fn ensure_generation(&self, generation: u64) -> Result<()> {
        if self.app_generation != Some(generation) {
            return Err(invalid_arg("Qt app is not active"));
        }

        Ok(())
    }

    fn allocate_node_id(&mut self) -> Result<(u32, u64)> {
        let generation = self
            .app_generation
            .ok_or_else(|| invalid_arg("call QtApp.start before creating nodes"))?;
        let id = self.next_node_id;
        self.next_node_id += 1;
        Ok((id, generation))
    }

    fn wrap_node(&mut self, id: u32) -> Result<QtNode> {
        let generation = self
            .app_generation
            .ok_or_else(|| invalid_arg("Qt app is not active"))?;
        let kind = self
            .tree
            .kind(id)
            .ok_or_else(|| invalid_arg(format!("node {id} not found")))?;

        if let Some(existing) = self.wrappers.get(&id).and_then(Weak::upgrade) {
            return Ok(QtNode::from_inner(existing));
        }

        let inner = Arc::new(QtNodeInner::new(id, generation, kind));
        self.wrappers.insert(id, Arc::downgrade(&inner));
        Ok(QtNode::from_inner(inner))
    }

    fn mark_destroyed(&mut self, id: u32) {
        if let Some(inner) = self.wrappers.get(&id).and_then(Weak::upgrade) {
            inner.mark_destroyed();
        }
        self.wrappers.remove(&id);
        crate::renderer::with_renderer_mut(|r| r.forget_node(id));
    }

    fn mark_destroyed_many(&mut self, ids: &[u32]) {
        for id in ids {
            self.mark_destroyed(*id);
        }
    }
}

static JS_CALLBACK: Lazy<Mutex<Option<Arc<EventCallback>>>> = Lazy::new(|| Mutex::new(None));
static CLEANUP_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
static RUNTIME_STATE: Lazy<Mutex<RuntimeState>> = Lazy::new(|| Mutex::new(RuntimeState::new()));

pub(crate) fn with_fragment_tree<T>(
    node_id: u32,
    run: impl FnOnce(&crate::canvas::fragment::FragmentTree) -> T,
) -> Option<T> {
    crate::renderer::with_renderer(|r| r.fragments.with(node_id, run))
}

pub(crate) fn with_fragment_tree_mut<T>(
    node_id: u32,
    run: impl FnOnce(&mut crate::canvas::fragment::FragmentTree) -> T,
) -> Option<T> {
    crate::renderer::with_renderer_mut(|r| r.fragments.with_mut(node_id, run))
}

pub(crate) fn ensure_fragment_tree(node_id: u32) {
    crate::renderer::with_renderer_mut(|r| r.fragments.ensure(node_id));
}

pub(crate) fn remove_fragment_tree(node_id: u32) {
    crate::renderer::with_renderer_mut(|r| r.fragments.remove(node_id));
}

pub(crate) fn qt_error(message: impl Into<String>) -> Error {
    Error::new(Status::GenericFailure, message.into())
}

pub(crate) fn invalid_arg(message: impl Into<String>) -> Error {
    Error::new(Status::InvalidArg, message.into())
}

pub(crate) fn install_cleanup_hook_once(env: &Env) -> Result<()> {
    if CLEANUP_HOOK_INSTALLED.swap(true, Ordering::SeqCst) {
        return Ok(());
    }

    env.add_env_cleanup_hook((), |_| {
        *JS_CALLBACK.lock().expect("js callback mutex poisoned") = None;
        RUNTIME_STATE
            .lock()
            .expect("runtime state mutex poisoned")
            .invalidate_all();
    })
    .map_err(|error| {
        CLEANUP_HOOK_INSTALLED.store(false, Ordering::SeqCst);
        qt_error(format!("failed to install N-API cleanup hook: {error}"))
    })?;

    Ok(())
}

pub(crate) fn emit_js_event(event: QtHostEvent) {
    let callback = JS_CALLBACK
        .lock()
        .expect("js callback mutex poisoned")
        .clone();

    let (trace_id, node_id, listener_id) = match &event {
        QtHostEvent::Listener {
            node_id,
            listener_id,
            trace_id,
            ..
        } => (
            trace_id.unwrap_or(0) as u64,
            Some(*node_id),
            Some(*listener_id),
        ),
        _ => (0, None, None),
    };

    let dispatch_mode = callback
        .as_ref()
        .map(|callback| callback.dispatch_mode().to_owned())
        .unwrap_or_else(|| "missing".to_owned());

    trace::record_static(
        trace_id,
        "rust",
        "rust.emit_js_event.call",
        node_id,
        listener_id,
        None,
        Some(dispatch_mode),
    );

    if let Some(callback) = callback {
        let _ = callback.call(event);
    }
}

fn set_js_callback(callback: Option<Arc<EventCallback>>) {
    *JS_CALLBACK.lock().expect("js callback mutex poisoned") = callback;
}

fn ensure_app_generation(generation: u64) -> Result<()> {
    let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(generation)
}

pub(crate) fn current_app_generation() -> Result<u64> {
    let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state
        .app_generation
        .ok_or_else(|| invalid_arg("Qt app is not active"))
}

pub(crate) fn ensure_live_node(node: &impl NodeHandle) -> Result<()> {
    if node.inner().is_destroyed() {
        return Err(invalid_arg(format!(
            "node {} has already been destroyed",
            node.inner().id
        )));
    }

    let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(node.inner().generation)?;

    if !state.tree.contains(node.inner().id) {
        return Err(invalid_arg(format!(
            "node {} is no longer attached",
            node.inner().id
        )));
    }

    Ok(())
}

fn wrap_optional_node(id: Option<u32>, generation: u64) -> Result<Option<QtNode>> {
    match id {
        Some(id) => {
            let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
            state.ensure_generation(generation)?;
            state.wrap_node(id).map(Some)
        }
        None => Ok(None),
    }
}

pub(crate) fn wrap_node_id(id: u32) -> Result<QtNode> {
    let generation = current_app_generation()?;
    let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(generation)?;
    state.wrap_node(id)
}

pub(crate) fn start_app(
    env: Env,
    on_event: Function<QtHostEvent, UnknownReturnValue>,
) -> Result<u64> {
    install_cleanup_hook_once(&env)?;

    {
        let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        if state.app_generation.is_some() || qt::qt_host_started() {
            return Err(invalid_arg("Qt app is already started"));
        }
    }

    let callback_ref = on_event.create_ref()?;
    let callback_tsfn = on_event.build_threadsafe_function().build()?;

    qt_host::start().map_err(|e| qt_error(e.to_string()))?;

    if let Err(error) = qt::qt_host_start(env.get_uv_event_loop()? as usize) {
        return Err(qt_error(error.what().to_owned()));
    }

    let generation = {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.start_new_app()
    };

    set_js_callback(Some(Arc::new(EventCallback::new(
        &env,
        callback_ref,
        callback_tsfn,
    ))));
    Ok(generation)
}

fn finish_shutdown_runtime_state() {
    RUNTIME_STATE
        .lock()
        .expect("runtime state mutex poisoned")
        .invalidate_all();
}

fn shutdown_host_now() -> Result<()> {
    qt::qt_host_shutdown().map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn shutdown_app(generation: u64) -> Result<()> {
    if !qt::qt_host_started() {
        return Ok(());
    }

    ensure_app_generation(generation)?;
    set_js_callback(None);

    shutdown_host_now()?;
    finish_shutdown_runtime_state();

    Ok(())
}

pub(crate) fn root_node(generation: u64) -> Result<QtNode> {
    ensure_app_generation(generation)?;

    let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.wrap_node(ROOT_NODE_ID)
}

pub(crate) fn node_by_id(generation: u64, node_id: u32) -> Result<QtNode> {
    ensure_app_generation(generation)?;

    let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(generation)?;
    state.wrap_node(node_id)
}

pub(crate) fn node_parent_id(generation: u64, node_id: u32) -> Result<Option<u32>> {
    ensure_app_generation(generation)?;

    let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(generation)?;
    if state.tree.kind(node_id).is_none() {
        return Err(invalid_arg(format!("node {node_id} not found")));
    }
    Ok(state.tree.get_parent(node_id))
}

pub(crate) fn subtree_node_ids(generation: u64, node_id: u32) -> Result<Vec<u32>> {
    ensure_app_generation(generation)?;

    let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(generation)?;
    state.tree.subtree_handles(node_id).map_err(invalid_arg)
}

pub(crate) fn create_widget_inner(generation: u64) -> Result<Arc<QtNodeInner>> {
    ensure_app_generation(generation)?;

    let (id, generation) = {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.allocate_node_id()?
    };

    qt::qt_create_widget(id, 1u8).map_err(|error| qt_error(error.what().to_owned()))?;

    let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(generation)?;
    state
        .tree
        .register(id, NodeKind::Window)
        .map_err(invalid_arg)?;
    let node = state.wrap_node(id)?;
    Ok(Arc::clone(node.inner()))
}

pub(crate) fn create_widget(generation: u64) -> Result<QtNode> {
    create_widget_inner(generation).map(QtNode::from_inner)
}

pub(crate) fn node_parent(node: &impl NodeHandle) -> Result<Option<QtNode>> {
    ensure_live_node(node)?;

    let parent_id = {
        let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.tree.get_parent(node.inner().id)
    };

    wrap_optional_node(parent_id, node.inner().generation)
}

pub(crate) fn node_first_child(node: &impl NodeHandle) -> Result<Option<QtNode>> {
    ensure_live_node(node)?;

    let child_id = {
        let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.tree.get_first_child(node.inner().id)
    };

    wrap_optional_node(child_id, node.inner().generation)
}

pub(crate) fn node_next_sibling(node: &impl NodeHandle) -> Result<Option<QtNode>> {
    ensure_live_node(node)?;

    let sibling_id = {
        let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.tree.get_next_sibling(node.inner().id)
    };

    wrap_optional_node(sibling_id, node.inner().generation)
}

pub(crate) fn node_is_text_node(_node: &impl NodeHandle) -> bool {
    false
}

pub(crate) fn insert_child(
    parent: &impl NodeHandle,
    child: &QtNode,
    anchor: Option<&QtNode>,
) -> Result<()> {
    ensure_live_node(parent)?;
    ensure_live_node(child)?;
    if let Some(anchor) = anchor {
        ensure_live_node(anchor)?;
    }

    let anchor_id = anchor.map(|node| node.inner().id);
    let anchor_id_or_zero = anchor_id.unwrap_or(0);

    {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(parent.inner().generation)?;
        state
            .tree
            .insert_child(parent.inner().id, child.inner().id, anchor_id)
            .map_err(invalid_arg)?;
    }

    qt::qt_insert_child(parent.inner().id, child.inner().id, anchor_id_or_zero)
        .map_err(|error| qt_error(error.what().to_owned()))?;

    if let Some(window_id) =
        scheduler::window_ancestor_id_for_node(parent.inner().generation, parent.inner().id)?
    {
        crate::renderer::with_renderer_mut(|r| {
            r.scheduler.mark_scene_subtree(window_id, parent.inner().id)
        });
    }
    Ok(())
}

pub(crate) fn remove_child(parent: &impl NodeHandle, child: &QtNode) -> Result<()> {
    ensure_live_node(parent)?;
    ensure_live_node(child)?;

    {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(parent.inner().generation)?;
        state
            .tree
            .remove_child(parent.inner().id, child.inner().id)
            .map_err(invalid_arg)?;
    }

    qt::qt_remove_child(parent.inner().id, child.inner().id)
        .map_err(|error| qt_error(error.what().to_owned()))?;

    if let Some(window_id) =
        scheduler::window_ancestor_id_for_node(parent.inner().generation, parent.inner().id)?
    {
        crate::renderer::with_renderer_mut(|r| {
            r.scheduler.mark_scene_subtree(window_id, parent.inner().id)
        });
    }
    Ok(())
}

pub(crate) fn destroy_node(node: &impl NodeHandle) -> Result<()> {
    ensure_live_node(node)?;
    if node.inner().kind.is_root() {
        return Err(invalid_arg("cannot destroy the renderer root node"));
    }

    if node.inner().mark_destroyed_once() {
        return Ok(());
    }

    let (removed_ids, parent_id, window_id) = {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(node.inner().generation)?;
        let parent_id = state.tree.get_parent(node.inner().id);
        let mut current = Some(node.inner().id);
        let mut window_id = None;
        while let Some(id) = current {
            let kind = state
                .tree
                .kind(id)
                .ok_or_else(|| invalid_arg(format!("node {id} not found")))?;
            if kind.is_window() {
                window_id = Some(id);
                break;
            }
            current = state.tree.get_parent(id);
        }
        let removed_ids = state
            .tree
            .remove_subtree(node.inner().id)
            .map_err(invalid_arg)?;
        (removed_ids, parent_id, window_id)
    };

    qt::qt_destroy_widget(node.inner().id, &removed_ids)
        .map_err(|error| qt_error(error.what().to_owned()))?;

    // Clean up renderer state (GPU surface, fragment tree, GPU mode) for this node.
    crate::renderer::with_renderer_mut(|r| r.destroy_window(node.inner().id));

    {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(node.inner().generation)?;
        state.mark_destroyed_many(&removed_ids);
    }
    if let Some(window_id) = window_id {
        let dirty_node_id = parent_id.unwrap_or(window_id);
        crate::renderer::with_renderer_mut(|r| {
            r.scheduler.mark_scene_subtree(window_id, dirty_node_id)
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{collections::HashSet, sync::Arc};

    use super::{
        capture::{WidgetCapture, WidgetCaptureFormat},
        tree::NodeTree,
        types::NodeKind,
    };
    use crate::renderer::scheduler::{
        WindowCaptureGrouping,
        pipeline::{coalesce_scene_subtree_roots_in_tree, group_window_capture_parts},
        state::WindowCaptureComposingPart,
    };

    fn capture_part(node_id: u32) -> WindowCaptureComposingPart {
        let capture =
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 10, 10, 40, 1.0)
                .unwrap();
        WindowCaptureComposingPart {
            node_id,
            x: 0,
            y: 0,
            width: 10,
            height: 10,
            capture: Arc::new(capture),
        }
    }

    #[test]
    fn segmented_window_capture_keeps_one_group_per_part() {
        let groups = group_window_capture_parts(
            WindowCaptureGrouping::Segmented,
            vec![capture_part(10), capture_part(11)],
        );

        assert_eq!(groups.len(), 2);
        assert_eq!(groups[0].len(), 1);
        assert_eq!(groups[0][0].node_id, 10);
        assert_eq!(groups[1].len(), 1);
        assert_eq!(groups[1][0].node_id, 11);
    }

    #[test]
    fn whole_window_capture_merges_parts_into_single_group() {
        let groups = group_window_capture_parts(
            WindowCaptureGrouping::WholeWindow,
            vec![capture_part(20), capture_part(21)],
        );

        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].len(), 2);
        assert_eq!(groups[0][0].node_id, 20);
        assert_eq!(groups[0][1].node_id, 21);
    }

    #[test]
    fn scene_subtree_roots_coalesce_to_minimal_ancestors() {
        let mut tree = NodeTree::with_root(1);
        tree.register(2, NodeKind::Window).expect("register");
        tree.register(3, NodeKind::Window).expect("register");
        tree.register(4, NodeKind::Window).expect("register");
        tree.insert_child(1, 2, None).expect("insert");
        tree.insert_child(2, 3, None).expect("insert");
        tree.insert_child(3, 4, None).expect("insert");

        let roots = HashSet::from([2_u32, 3_u32, 4_u32]);
        let minimal = coalesce_scene_subtree_roots_in_tree(&tree, &roots);

        assert_eq!(minimal, HashSet::from([2_u32]));
    }

    #[test]
    fn dirty_node_marks_compositor_node() {
        use crate::renderer::scheduler::Scheduler;
        let mut scheduler = Scheduler::new();

        scheduler.mark_dirty_node(7, 9);

        assert_eq!(
            scheduler.dirty_nodes_for_test(7),
            Some(&HashSet::from([9_u32]))
        );
    }

    #[test]
    fn geometry_node_tracking_keeps_dirty_state_separate() {
        use crate::renderer::scheduler::Scheduler;
        let mut scheduler = Scheduler::new();

        scheduler.mark_geometry_node(7, 9);

        assert_eq!(scheduler.take_geometry_nodes(7), HashSet::from([9_u32]));
    }
}
