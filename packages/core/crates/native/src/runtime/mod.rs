pub mod capture;
pub mod tree;
pub mod types;

use std::{
    collections::HashMap,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex, Weak,
    },
    thread::{self, ThreadId},
};

use napi::{
    bindgen_prelude::{Function, FunctionRef},
    threadsafe_function::{ThreadsafeFunction, ThreadsafeFunctionCallMode, UnknownReturnValue},
    Env, Error, Result, Status,
};
use once_cell::sync::Lazy;

use self::capture::WidgetCapture;
use self::tree::NodeTree;
use self::types::NodeKind;
#[rustfmt::skip]
use ::window_host::HostCapabilities as RawWindowHostCapabilities;

use crate::{
    api::{
        AlignItems, FlexDirection, JustifyContent, QtDebugNodeBounds, QtDebugNodeSnapshot,
        QtDebugSnapshot, QtHostEvent, QtNode, QtWindowCaptureFrame,
        QtWindowFrameState, QtWindowHostCapabilities, QtWindowHostInfo, WindowPropUpdate,
    },
    qt::{self, QtRealizedNodeState},
    trace,
    window_compositor::{self, CompositorState},
    window_host,
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
    compositor: CompositorState,
    fragment_trees: HashMap<u32, crate::canvas::fragment::FragmentTree>,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            generation_counter: 0,
            app_generation: None,
            next_node_id: ROOT_NODE_ID + 1,
            tree: NodeTree::with_root(ROOT_NODE_ID),
            wrappers: HashMap::new(),
            compositor: CompositorState::new(),
            fragment_trees: HashMap::new(),
        }
    }

    fn start_new_app(&mut self) -> u64 {
        self.generation_counter += 1;
        let generation = self.generation_counter;
        self.app_generation = Some(generation);
        self.next_node_id = ROOT_NODE_ID + 1;
        self.tree.reset_with_root(ROOT_NODE_ID);
        self.wrappers.clear();
        self.compositor.clear_all();
        self.fragment_trees.clear();
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
        self.compositor.clear_all();
        self.fragment_trees.clear();
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
        self.fragment_trees.remove(&id);
        self.compositor.clear_all();
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
static WINDOW_GPU_MODE: Lazy<Mutex<HashMap<u32, bool>>> =
    Lazy::new(|| Mutex::new(HashMap::new()));

pub(crate) fn set_window_gpu_mode(node_id: u32, gpu: bool) {
    WINDOW_GPU_MODE
        .lock()
        .expect("window gpu mode mutex poisoned")
        .insert(node_id, gpu);
}

pub(crate) fn window_gpu_enabled(node_id: u32) -> bool {
    WINDOW_GPU_MODE
        .lock()
        .expect("window gpu mode mutex poisoned")
        .get(&node_id)
        .copied()
        .unwrap_or(false)
}

fn with_runtime_state<T>(run: impl FnOnce(&RuntimeState) -> T) -> T {
    let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    run(&state)
}

fn with_runtime_state_mut<T>(run: impl FnOnce(&mut RuntimeState) -> T) -> T {
    let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    run(&mut state)
}

pub(crate) fn with_compositor_state<T>(run: impl FnOnce(&CompositorState) -> T) -> T {
    with_runtime_state(|state| run(&state.compositor))
}

pub(crate) fn with_compositor_state_mut<T>(run: impl FnOnce(&mut CompositorState) -> T) -> T {
    with_runtime_state_mut(|state| run(&mut state.compositor))
}

pub(crate) fn with_fragment_tree<T>(
    node_id: u32,
    run: impl FnOnce(&crate::canvas::fragment::FragmentTree) -> T,
) -> Option<T> {
    with_runtime_state(|state| state.fragment_trees.get(&node_id).map(|tree| run(tree)))
}

pub(crate) fn with_fragment_tree_mut<T>(
    node_id: u32,
    run: impl FnOnce(&mut crate::canvas::fragment::FragmentTree) -> T,
) -> Option<T> {
    with_runtime_state_mut(|state| state.fragment_trees.get_mut(&node_id).map(|tree| run(tree)))
}

pub(crate) fn ensure_fragment_tree(node_id: u32) {
    with_runtime_state_mut(|state| {
        state
            .fragment_trees
            .entry(node_id)
            .or_insert_with(crate::canvas::fragment::FragmentTree::new);
    });
}

pub(crate) fn remove_fragment_tree(node_id: u32) {
    with_runtime_state_mut(|state| {
        state.fragment_trees.remove(&node_id);
    });
}

pub(crate) fn qt_error(message: impl Into<String>) -> Error {
    Error::new(Status::GenericFailure, message.into())
}

pub(crate) fn invalid_arg(message: impl Into<String>) -> Error {
    Error::new(Status::InvalidArg, message.into())
}

fn flex_direction_from_tag(tag: u8) -> Option<FlexDirection> {
    match tag {
        1 => Some(FlexDirection::Column),
        2 => Some(FlexDirection::Row),
        _ => None,
    }
}

fn align_items_from_tag(tag: u8) -> Option<AlignItems> {
    match tag {
        1 => Some(AlignItems::FlexStart),
        2 => Some(AlignItems::Center),
        3 => Some(AlignItems::FlexEnd),
        4 => Some(AlignItems::Stretch),
        _ => None,
    }
}

fn justify_content_from_tag(tag: u8) -> Option<JustifyContent> {
    match tag {
        1 => Some(JustifyContent::FlexStart),
        2 => Some(JustifyContent::Center),
        3 => Some(JustifyContent::FlexEnd),
        _ => None,
    }
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
        _ => {
            (0, None, None)
        }
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

    window_host::start()?;

    if let Err(error) = qt::start_qt_host(env.get_uv_event_loop()? as usize) {
        window_host::stop();
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
    qt::shutdown_qt_host().map_err(|error| qt_error(error.what().to_owned()))?;
    window_host::stop();
    Ok(())
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

pub(crate) fn create_widget_inner(
    generation: u64,
) -> Result<Arc<QtNodeInner>> {
    ensure_app_generation(generation)?;

    let (id, generation) = {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.allocate_node_id()?
    };

    qt::qt_create_widget(id, 1u8)
        .map_err(|error| qt_error(error.what().to_owned()))?;

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

fn api_window_host_capabilities(
    capabilities: RawWindowHostCapabilities,
) -> QtWindowHostCapabilities {
    QtWindowHostCapabilities {
        backend_kind: capabilities.backend_kind.to_string(),
        supports_zero_timeout_pump: capabilities.supports_zero_timeout_pump,
        supports_external_wake: capabilities.supports_external_wake,
        supports_fd_bridge: capabilities.supports_fd_bridge,
    }
}

fn current_window_host_backend_name() -> String {
    crate::window_host::backend_name().unwrap_or_else(crate::window_host::detected_backend_name)
}

fn current_window_host_capabilities() -> QtWindowHostCapabilities {
    api_window_host_capabilities(
        crate::window_host::capabilities()
            .unwrap_or_else(crate::window_host::detected_capabilities),
    )
}

pub(crate) fn window_host_info() -> QtWindowHostInfo {
    QtWindowHostInfo {
        enabled: window_host::enabled(),
        backend_name: current_window_host_backend_name(),
        capabilities: current_window_host_capabilities(),
    }
}

pub(crate) fn debug_snapshot(generation: u64) -> Result<QtDebugSnapshot> {
    ensure_app_generation(generation)?;

    let nodes_to_snapshot = {
        let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(generation)?;
        let mut nodes = Vec::new();
        for id in state.tree.all_handles() {
            let kind = state
                .tree
                .kind(id)
                .ok_or_else(|| invalid_arg(format!("node {id} not found")))?;
            let parent_id = state.tree.get_parent(id);
            let children = state.tree.children(id).unwrap_or(&[]).to_vec();
            nodes.push((id, kind, parent_id, children));
        }
        nodes
    };

    let mut nodes = Vec::new();
    for (id, kind, parent_id, children) in nodes_to_snapshot {
        if kind.is_root() {
            nodes.push(QtDebugNodeSnapshot {
                id,
                kind: kind.label().to_owned(),
                parent_id,
                children,
                text: None,
                title: None,
                width: None,
                height: None,
                min_width: None,
                min_height: None,
                flex_grow: None,
                flex_shrink: None,
                enabled: None,
                placeholder: None,
                checked: None,
                flex_direction: None,
                justify_content: None,
                align_items: None,
                gap: None,
                padding: None,
                value: None,
            });
            continue;
        }

        let realized = qt::qt_debug_node_state(id);
        let snapshot = snapshot_from_realized_state(id, parent_id, children, realized);
        nodes.push(snapshot);
    }

    let window_host_backend = Some(current_window_host_backend_name());
    let window_host_capabilities = Some(current_window_host_capabilities());

    Ok(QtDebugSnapshot {
        host_runtime: "nodejs".to_owned(),
        window_host_backend,
        window_host_capabilities,
        root_id: ROOT_NODE_ID,
        nodes,
    })
}

fn snapshot_from_realized_state(
    id: u32,
    parent_id: Option<u32>,
    children: Vec<u32>,
    realized: QtRealizedNodeState,
) -> QtDebugNodeSnapshot {
    QtDebugNodeSnapshot {
        id,
        kind: "window".to_owned(),
        parent_id,
        children,
        text: realized.has_text.then_some(realized.text),
        title: realized.has_title.then_some(realized.title),
        width: realized.has_width.then_some(realized.width),
        height: realized.has_height.then_some(realized.height),
        min_width: realized.has_min_width.then_some(realized.min_width),
        min_height: realized.has_min_height.then_some(realized.min_height),
        flex_grow: realized.has_flex_grow.then_some(realized.flex_grow),
        flex_shrink: realized.has_flex_shrink.then_some(realized.flex_shrink),
        enabled: realized.has_enabled.then_some(realized.enabled),
        placeholder: realized.has_placeholder.then_some(realized.placeholder),
        checked: realized.has_checked.then_some(realized.checked),
        flex_direction: flex_direction_from_tag(realized.flex_direction_tag),
        justify_content: justify_content_from_tag(realized.justify_content_tag),
        align_items: align_items_from_tag(realized.align_items_tag),
        gap: realized.has_gap.then_some(realized.gap),
        padding: realized.has_padding.then_some(realized.padding),
        value: realized.has_value.then_some(realized.value),
    }
}



fn read_window_screen_x(node: &impl NodeHandle) -> Result<i32> {
    let bounds = qt::debug_node_bounds(node.inner().id);
    Ok(bounds.screen_x)
}

fn read_window_screen_y(node: &impl NodeHandle) -> Result<i32> {
    let bounds = qt::debug_node_bounds(node.inner().id);
    Ok(bounds.screen_y)
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

    if let Some(window_id) = window_compositor::window_ancestor_id_for_node(
        parent.inner().generation,
        parent.inner().id,
    )? {
        window_compositor::mark_window_compositor_scene_subtree(window_id, parent.inner().id);
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

    if let Some(window_id) = window_compositor::window_ancestor_id_for_node(
        parent.inner().generation,
        parent.inner().id,
    )? {
        window_compositor::mark_window_compositor_scene_subtree(window_id, parent.inner().id);
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

    // Clean up renderer state (CPU pixmap buffer + GPU surface) for this node.
    // No-op for non-window nodes since they have no entries in the maps.
    crate::surface_renderer::destroy_window_renderer_state(node.inner().id);
    WINDOW_GPU_MODE.lock().expect("window gpu mode mutex poisoned").remove(&node.inner().id);

    {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(node.inner().generation)?;
        state.mark_destroyed_many(&removed_ids);
    }
    if let Some(window_id) = window_id {
        let dirty_node_id = parent_id.unwrap_or(window_id);
        window_compositor::mark_window_compositor_scene_subtree(window_id, dirty_node_id);
    }
    Ok(())
}

pub(crate) fn request_repaint(node: &impl NodeHandle) -> Result<()> {
    ensure_live_node(node)?;
    if let Some(window_id) =
        window_compositor::window_ancestor_id_for_node(node.inner().generation, node.inner().id)?
    {
        window_compositor::qt_mark_window_compositor_pixels_dirty(window_id, node.inner().id);
    }
    let _ = qt::qt_request_window_compositor_frame(node.inner().id);
    qt::qt_request_repaint(node.inner().id).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn request_window_repaint_exact(window: &impl NodeHandle) -> Result<()> {
    ensure_live_node(window)?;
    qt::qt_request_repaint(window.inner().id).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn request_overlay_next_frame_exact(
    window: &impl NodeHandle,
    overlay_node_id: u32,
) -> Result<()> {
    ensure_live_node(window)?;
    window_compositor::mark_window_compositor_frame_tick_node(window.inner().id, overlay_node_id);
    if qt::qt_request_window_compositor_frame(window.inner().id)
        .map_err(|error| qt_error(error.what().to_owned()))?
    {
        Ok(())
    } else {
        request_window_repaint_exact(window)
    }
}

pub(crate) fn capture_widget_exact(node: &impl NodeHandle) -> Result<WidgetCapture> {
    ensure_live_node(node)?;
    if node.inner().is_window() {
        return window_compositor::capture_window_widget_exact(node);
    }

    window_compositor::capture_painted_widget_exact_with_children(node, true)
}

pub(crate) fn wire_event(node: &impl NodeHandle, export_id: u16) -> Result<()> {
    ensure_live_node(node)?;

    let export_name = match export_id {
        1 => "onCloseRequested",
        2 => "onHoverEnter",
        3 => "onHoverLeave",
        _ => return Err(invalid_arg(format!("unknown event export id {export_id}"))),
    };

    if node.inner().is_window() {
        let id = node.inner().id;
        return match export_name {
            "onCloseRequested" => qt::qt_window_wire_close_requested(id)
                .map_err(|e| qt_error(e.what().to_owned())),
            "onHoverEnter" => qt::qt_window_wire_hover_enter(id)
                .map_err(|e| qt_error(e.what().to_owned())),
            "onHoverLeave" => qt::qt_window_wire_hover_leave(id)
                .map_err(|e| qt_error(e.what().to_owned())),
            _ => Err(invalid_arg(format!(
                "unknown window event export {export_name}"
            ))),
        };
    }

    Err(invalid_arg(format!(
        "event export {export_name} not supported on widget kind {}",
        node.inner().kind.label()
    )))
}

pub(crate) fn apply_prop(node: &impl NodeHandle, update: WindowPropUpdate) -> Result<()> {
    ensure_live_node(node)?;
    let id = node.inner().id;
    match update {
        WindowPropUpdate::Title { value } => {
            qt::qt_window_set_title(id, &value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Width { value } => {
            qt::qt_window_set_width(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Height { value } => {
            qt::qt_window_set_height(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::MinWidth { value } => {
            qt::qt_window_set_min_width(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::MinHeight { value } => {
            qt::qt_window_set_min_height(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Visible { value } => {
            qt::qt_window_set_visible(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Enabled { value } => {
            qt::qt_window_set_enabled(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Frameless { value } => {
            qt::qt_window_set_frameless(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::TransparentBackground { value } => {
            qt::qt_window_set_transparent_background(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::AlwaysOnTop { value } => {
            qt::qt_window_set_always_on_top(id, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Gpu { value } => {
            set_window_gpu_mode(id, value);
        }
        WindowPropUpdate::WindowKind { value } => {
            let tag = u8::try_from(value).map_err(|_| invalid_arg("windowKind out of range"))?;
            qt::qt_window_set_window_kind(id, tag)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::ScreenX { value } => {
            let y = read_window_screen_y(node)?;
            qt::qt_window_set_screen_position(id, value, y)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::ScreenY { value } => {
            let x = read_window_screen_x(node)?;
            qt::qt_window_set_screen_position(id, x, value)
                .map_err(|e| qt_error(e.what().to_owned()))?;
        }
        WindowPropUpdate::Text { .. } => {
            return Ok(());
        }
    }
    Ok(())
}

pub(crate) fn schedule_debug_event(delay_ms: u32, event: String) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before scheduling a debug event",
        ));
    }

    qt::schedule_debug_event(delay_ms, event.as_str())
        .map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_click_node(node_id: u32) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before triggering debug clicks",
        ));
    }

    qt::debug_click_node(node_id).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_close_node(node_id: u32) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before triggering debug close requests",
        ));
    }

    qt::debug_close_node(node_id).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_emit_app_event(name: String) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before triggering debug app events",
        ));
    }

    emit_app_event(name.as_str());
    Ok(())
}

pub(crate) fn request_next_frame_exact(node: &impl NodeHandle) -> Result<()> {
    window_compositor::write_frame_bool_prop(node, "nextFrameRequested", true)?;
    request_window_repaint_exact(node)
}

pub(crate) fn read_window_frame_state_exact(node: &impl NodeHandle) -> Result<QtWindowFrameState> {
    Ok(QtWindowFrameState {
        seq: window_compositor::read_frame_f64_prop(node, "seq")?,
        elapsed_ms: window_compositor::read_frame_f64_prop(node, "elapsedMs")?,
        delta_ms: window_compositor::read_frame_f64_prop(node, "deltaMs")?,
    })
}

pub(crate) fn debug_input_insert_text(node_id: u32, value: String) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before triggering debug text input",
        ));
    }

    qt::debug_input_insert_text(node_id, value.as_str())
        .map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_highlight_node(node_id: u32) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before triggering debug highlight",
        ));
    }

    qt::debug_highlight_node(node_id).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_node_bounds(node_id: u32) -> Result<QtDebugNodeBounds> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before reading debug node bounds",
        ));
    }

    let bounds = qt::debug_node_bounds(node_id);
    Ok(QtDebugNodeBounds {
        visible: bounds.visible,
        screen_x: bounds.screen_x,
        screen_y: bounds.screen_y,
        width: bounds.width,
        height: bounds.height,
    })
}

pub(crate) fn debug_node_at_point(screen_x: i32, screen_y: i32) -> Result<Option<u32>> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before reading debug node at point",
        ));
    }

    let node_id = qt::debug_node_at_point(screen_x, screen_y);
    Ok((node_id != 0).then_some(node_id))
}

pub(crate) fn debug_capture_window_frame(window_id: u32) -> Result<QtWindowCaptureFrame> {
    window_compositor::capture_window_frame_exact(
        window_id,
        window_compositor::WindowCaptureGrouping::Segmented,
    )?
    .into_api_frame()
}

pub(crate) fn debug_set_inspect_mode(enabled: bool) -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before toggling debug inspect mode",
        ));
    }

    qt::debug_set_inspect_mode(enabled).map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn debug_clear_highlight() -> Result<()> {
    if !qt::qt_host_started() {
        return Err(invalid_arg(
            "call QtApp.start before clearing debug highlight",
        ));
    }

    qt::debug_clear_highlight().map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn emit_app_event(name: &str) {
    emit_js_event(QtHostEvent::App {
        name: name.to_owned(),
    });
}

pub(crate) fn emit_debug_event(name: &str) {
    emit_js_event(QtHostEvent::Debug {
        name: name.to_owned(),
    });
}

pub(crate) fn emit_inspect_event(node_id: u32) {
    emit_js_event(QtHostEvent::Inspect { node_id });
}

pub(crate) fn emit_canvas_pointer_event(canvas_node_id: u32, event_tag: u8, x: f64, y: f64) {
    use crate::canvas::fragment::{fragment_store_hit_test, fragment_store_get_cursor, fragment_store_focus_fragment};

    let fragment_id_opt = fragment_store_hit_test(canvas_node_id, x, y);
    let fragment_id = fragment_id_opt
        .map(|id| id.0 as i32)
        .unwrap_or(-1);

    // Update cursor on mouse move.
    if event_tag == 3 {
        let cursor_tag = fragment_id_opt
            .map(|id| fragment_store_get_cursor(canvas_node_id, id))
            .unwrap_or(0);
        let _ = crate::qt::ffi::qt_canvas_set_cursor(canvas_node_id, cursor_tag);
    }

    // Auto-focus on press.
    if event_tag == 1 {
        if let Some(fid) = fragment_id_opt {
            let (old, new) = fragment_store_focus_fragment(canvas_node_id, fid);
            if old != new {
                emit_js_event(QtHostEvent::CanvasFocusChange {
                    canvas_node_id,
                    old_fragment_id: old,
                    new_fragment_id: new,
                });
            }
            // Click-to-cursor: place caret in TextInput at click x position.
            crate::canvas::fragment::fragment_store_click_to_cursor(canvas_node_id, fid, x, y);
        }
        crate::qt::ffi::sync_text_edit_session_for_focus(canvas_node_id);
    }

    // Drag-select: extend selection while left button held (tag 4 from C++).
    if event_tag == 4 {
        crate::canvas::fragment::fragment_store_drag_to_cursor(canvas_node_id, x, y);
    }

    emit_js_event(QtHostEvent::CanvasPointer {
        canvas_node_id,
        fragment_id,
        event_tag,
        x,
        y,
    });
}

pub(crate) fn qt_canvas_key_event(
    canvas_node_id: u32,
    event_tag: u8,
    qt_key: i32,
    modifiers: u32,
    text: &str,
    repeat: bool,
    native_scan_code: u32,
    native_virtual_key: u32,
) {
    let fragment_id = crate::canvas::fragment::fragment_store_focused(canvas_node_id);

    emit_js_event(QtHostEvent::CanvasKeyboard {
        canvas_node_id,
        fragment_id,
        event_tag,
        qt_key,
        modifiers,
        text: text.to_owned(),
        repeat,
        native_scan_code,
        native_virtual_key,
    });
}

pub(crate) fn qt_canvas_wheel_event(
    canvas_node_id: u32,
    delta_x: f64,
    delta_y: f64,
    pixel_dx: f64,
    pixel_dy: f64,
    x: f64,
    y: f64,
    modifiers: u32,
    phase: u32,
) {
    use crate::canvas::fragment::fragment_store_hit_test;

    let fragment_id = fragment_store_hit_test(canvas_node_id, x, y)
        .map(|id| id.0 as i32)
        .unwrap_or(-1);

    emit_js_event(QtHostEvent::CanvasWheel {
        canvas_node_id,
        fragment_id,
        delta_x,
        delta_y,
        pixel_dx,
        pixel_dy,
        x,
        y,
        modifiers,
        phase,
    });
}

pub(crate) fn emit_window_typed_event(node_id: u32, export_name: &str) {
    let export_id = match export_name {
        "onCloseRequested" => 1u16,
        "onHoverEnter" => 2,
        "onHoverLeave" => 3,
        _ => return,
    };
    emit_js_event(QtHostEvent::Listener {
        node_id,
        listener_id: export_id,
        trace_id: None,
    });
}

pub(crate) fn qt_window_event_focus_change(node_id: u32, gained: bool) {
    emit_js_event(QtHostEvent::WindowFocusChange { node_id, gained });
}

pub(crate) fn qt_window_event_resize(node_id: u32, width: f64, height: f64) {
    emit_js_event(QtHostEvent::WindowResize { node_id, width, height });
}

pub(crate) fn qt_window_event_state_change(node_id: u32, state: u8) {
    emit_js_event(QtHostEvent::WindowStateChange { node_id, state });
}

pub(crate) fn qt_system_color_scheme_changed(scheme: u8) {
    let scheme_str = match scheme {
        1 => "light",
        2 => "dark",
        _ => "unknown",
    };
    emit_js_event(QtHostEvent::ColorSchemeChange { scheme: scheme_str.to_owned() });
}

pub(crate) fn qt_screen_dpi_changed(dpi: f64) {
    emit_js_event(QtHostEvent::ScreenDpiChange { dpi });
}

pub(crate) fn qt_file_dialog_result(request_id: u32, paths: Vec<String>) {
    emit_js_event(QtHostEvent::FileDialogResult { request_id, paths });
}

#[cfg(test)]
mod tests {
    use std::{
        collections::HashSet,
        sync::Arc,
    };

    use super::RuntimeState;
    use crate::window_compositor::{
        pipeline::{
            coalesce_scene_subtree_roots_in_tree, group_window_capture_parts,
        },
        state::WindowCaptureComposingPart,
        WindowCaptureGrouping,
    };
    use super::{
        capture::{WidgetCapture, WidgetCaptureFormat},
        tree::NodeTree,
        types::NodeKind,
    };

    fn capture_part(node_id: u32) -> WindowCaptureComposingPart {
        let capture = WidgetCapture::new_zeroed(
            WidgetCaptureFormat::Argb32Premultiplied,
            10,
            10,
            40,
            1.0,
        )
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
        tree.register(2, NodeKind::Window)
            .expect("register");
        tree.register(3, NodeKind::Window)
            .expect("register");
        tree.register(4, NodeKind::Window)
            .expect("register");
        tree.insert_child(1, 2, None).expect("insert");
        tree.insert_child(2, 3, None).expect("insert");
        tree.insert_child(3, 4, None).expect("insert");

        let roots = HashSet::from([2_u32, 3_u32, 4_u32]);
        let minimal = coalesce_scene_subtree_roots_in_tree(&tree, &roots);

        assert_eq!(minimal, HashSet::from([2_u32]));
    }

    #[test]
    fn dirty_node_marks_compositor_node() {
        let mut state = RuntimeState::new();
        state.app_generation = Some(1);

        state.compositor.mark_dirty_node(7, 9);

        assert_eq!(
            state.compositor.dirty_nodes_for_test(7),
            Some(&HashSet::from([9_u32]))
        );
    }

    #[test]
    fn geometry_node_tracking_keeps_dirty_state_separate() {
        let mut state = RuntimeState::new();
        state.app_generation = Some(1);

        state.compositor.mark_geometry_node(7, 9);

        assert_eq!(
            state.compositor.take_geometry_nodes(7),
            HashSet::from([9_u32])
        );
    }
}
