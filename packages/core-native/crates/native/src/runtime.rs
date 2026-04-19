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
use qt_solid_runtime::tree::NodeTree;
use qt_solid_widget_core::{
    decl::{NodeClass, WidgetTypeId},
    runtime::{self as widget_runtime, QtOpaqueBorrow, QtOpaqueInfo, QtValue, WidgetCapture},
    schema::{QtTypeInfo, QtValueRepr, SpecCreateProp, enum_tag_for_value, merged_props},
};
#[rustfmt::skip]
use ::window_host::HostCapabilities as RawWindowHostCapabilities;

use crate::{
    api::{
        AlignItems, FlexDirection, JustifyContent, QtDebugNodeBounds, QtDebugNodeSnapshot,
        QtDebugSnapshot, QtHostEvent, QtInitialProp, QtListenerValue, QtNode, QtWindowCaptureFrame,
        QtWindowFrameState, QtWindowHostCapabilities, QtWindowHostInfo,
    },
    bootstrap::widget_registry,
    qt::{self, QtRealizedNodeState},
    trace,
    window_compositor::{self, CompositorState},
    window_host,
};

pub(crate) const ROOT_NODE_ID: u32 = 1;

type EventCallbackTsfn =
    ThreadsafeFunction<QtHostEvent, UnknownReturnValue, QtHostEvent, Status, false>;
type EventCallbackRef = FunctionRef<QtHostEvent, UnknownReturnValue>;
type ListenerPayload = Arc<[QtListenerValue]>;

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
    pub(crate) class: NodeClass,
    destroyed: AtomicBool,
}

impl QtNodeInner {
    pub(crate) fn new(id: u32, generation: u64, class: NodeClass) -> Self {
        Self {
            id,
            generation,
            class,
            destroyed: AtomicBool::new(false),
        }
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

    pub(crate) fn binding(&self) -> &'static crate::bootstrap::WidgetBinding {
        widget_registry().binding_for_node_class(self.class)
    }
}

pub(crate) trait NodeHandle {
    fn inner(&self) -> &Arc<QtNodeInner>;
}

#[derive(Clone)]
struct NativeWidgetRuntimeHandle {
    inner: Arc<QtNodeInner>,
}

impl widget_runtime::WidgetRuntimeHandle for NativeWidgetRuntimeHandle {
    fn apply_prop_path(&self, path: &str, value: QtValue) -> widget_runtime::WidgetResult<()> {
        let node = NativeNodeHandleRef {
            inner: Arc::clone(&self.inner),
        };
        apply_qt_prop_by_name(&node, path, value)
            .map_err(|error| widget_runtime::WidgetError::new(error.to_string()))
    }

    fn call_host_method(
        &self,
        name: &str,
        args: &[QtValue],
    ) -> widget_runtime::WidgetResult<QtValue> {
        let node = NativeNodeHandleRef {
            inner: Arc::clone(&self.inner),
        };
        call_host_method_exact(&node, name, args.to_vec())
            .map_err(|error| widget_runtime::WidgetError::new(error.to_string()))
    }

    fn request_repaint(&self) -> widget_runtime::WidgetResult<()> {
        let node = NativeNodeHandleRef {
            inner: Arc::clone(&self.inner),
        };
        request_repaint_exact(&node)
            .map_err(|error| widget_runtime::WidgetError::new(error.to_string()))
    }

    fn capture(&self) -> widget_runtime::WidgetResult<WidgetCapture> {
        let node = NativeNodeHandleRef {
            inner: Arc::clone(&self.inner),
        };
        capture_widget_exact(&node)
            .map_err(|error| widget_runtime::WidgetError::new(error.to_string()))
    }
}

struct NativeNodeHandleRef {
    inner: Arc<QtNodeInner>,
}

impl NodeHandle for NativeNodeHandleRef {
    fn inner(&self) -> &Arc<QtNodeInner> {
        &self.inner
    }
}

fn lower_initial_prop_value(
    prop: QtInitialProp,
    spec: &SpecCreateProp,
) -> Result<widget_runtime::WidgetCreateProp> {
    let value = match spec.value_type.repr() {
        QtValueRepr::String => QtValue::String(prop.string_value.ok_or_else(|| {
            invalid_arg(format!("missing string value for create prop {}", spec.key))
        })?),
        QtValueRepr::Bool => QtValue::Bool(prop.bool_value.ok_or_else(|| {
            invalid_arg(format!(
                "missing boolean value for create prop {}",
                spec.key
            ))
        })?),
        QtValueRepr::I32 { non_negative } => {
            let value = prop.i32_value.ok_or_else(|| {
                invalid_arg(format!("missing i32 value for create prop {}", spec.key))
            })?;
            if non_negative && value < 0 {
                return Err(invalid_arg(format!(
                    "create prop {} must be non-negative",
                    spec.key
                )));
            }
            QtValue::I32(value)
        }
        QtValueRepr::F64 { non_negative } => {
            let value = prop.f64_value.ok_or_else(|| {
                invalid_arg(format!("missing f64 value for create prop {}", spec.key))
            })?;
            if non_negative && value < 0.0 {
                return Err(invalid_arg(format!(
                    "create prop {} must be non-negative",
                    spec.key
                )));
            }
            QtValue::F64(value)
        }
        QtValueRepr::Enum(domain) => {
            let value = prop.string_value.ok_or_else(|| {
                invalid_arg(format!("missing enum value for create prop {}", spec.key))
            })?;
            let tag = enum_tag_for_value(domain, &value).ok_or_else(|| {
                invalid_arg(format!(
                    "invalid {} value {} for create prop {}",
                    domain.name, value, spec.key
                ))
            })?;
            QtValue::Enum(tag)
        }
        other => {
            return Err(invalid_arg(format!(
                "unsupported create prop {} type {:?}",
                spec.key, other
            )));
        }
    };

    Ok(widget_runtime::WidgetCreateProp {
        key: spec.key.to_owned(),
        value,
    })
}

fn lower_widget_create_props(
    widget_type_id: WidgetTypeId,
    initial_props: Vec<QtInitialProp>,
) -> Result<Vec<widget_runtime::WidgetCreateProp>> {
    let Some(decl) = widget_registry().prop_decl_for_widget_type_id(widget_type_id) else {
        if initial_props.is_empty() {
            return Ok(Vec::new());
        }
        return Err(invalid_arg(format!(
            "widget {} does not accept constructor props",
            widget_registry().binding(widget_type_id).kind_name
        )));
    };

    let mut create_props = Vec::with_capacity(initial_props.len());
    for prop in initial_props {
        let spec = decl
            .create_props
            .iter()
            .find(|candidate| candidate.key == prop.key)
            .ok_or_else(|| {
                invalid_arg(format!(
                    "unknown constructor prop {} for {}",
                    prop.key,
                    widget_registry().binding(widget_type_id).kind_name
                ))
            })?;
        create_props.push(lower_initial_prop_value(prop, spec)?);
    }

    Ok(create_props)
}

struct QPainterOpaqueHost<'a> {
    raw: std::pin::Pin<&'a mut crate::qt::QPainter>,
}

impl widget_runtime::QtOpaqueHostDyn for QPainterOpaqueHost<'_> {
    fn opaque_info(&self) -> QtOpaqueInfo {
        QtOpaqueInfo::new(
            "native::qt::QPainter",
            "QPainter",
            "<QtGui/QPainter>",
            QtOpaqueBorrow::Mut,
        )
    }
}

impl widget_runtime::QtOpaqueHostMutDyn for QPainterOpaqueHost<'_> {
    fn call_host_slot_mut(
        &mut self,
        slot: u16,
        args: &[QtValue],
    ) -> widget_runtime::WidgetResult<QtValue> {
        let args = args
            .iter()
            .cloned()
            .map(qt_value_to_method_value_core)
            .collect::<widget_runtime::WidgetResult<Vec<_>>>()?;
        let value = crate::qt::qt_qpainter_call(self.raw.as_mut(), slot, &args)
            .map_err(|error| widget_runtime::WidgetError::new(error.what().to_owned()))?;
        method_value_to_qt_value(value)
            .map_err(|error| widget_runtime::WidgetError::new(error.to_string()))
    }
}

struct RuntimeState {
    generation_counter: u64,
    app_generation: Option<u64>,
    next_node_id: u32,
    tree: NodeTree,
    wrappers: HashMap<u32, Weak<QtNodeInner>>,
    widget_instances: HashMap<u32, Arc<dyn widget_runtime::QtWidgetInstanceDyn>>,
    compositor: CompositorState,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            generation_counter: 0,
            app_generation: None,
            next_node_id: ROOT_NODE_ID + 1,
            tree: NodeTree::with_root(ROOT_NODE_ID),
            wrappers: HashMap::new(),
            widget_instances: HashMap::new(),
            compositor: CompositorState::new(),
        }
    }

    fn start_new_app(&mut self) -> u64 {
        self.generation_counter += 1;
        let generation = self.generation_counter;
        self.app_generation = Some(generation);
        self.next_node_id = ROOT_NODE_ID + 1;
        self.tree.reset_with_root(ROOT_NODE_ID);
        self.wrappers.clear();
        self.widget_instances.clear();
        self.compositor.clear_all();
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
        self.widget_instances.clear();
        self.compositor.clear_all();
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
        let class = self
            .tree
            .class(id)
            .ok_or_else(|| invalid_arg(format!("node {id} not found")))?;

        if let Some(existing) = self.wrappers.get(&id).and_then(Weak::upgrade) {
            return Ok(QtNode::from_inner(existing));
        }

        let inner = Arc::new(QtNodeInner::new(id, generation, class));
        self.wrappers.insert(id, Arc::downgrade(&inner));
        Ok(QtNode::from_inner(inner))
    }

    fn mark_destroyed(&mut self, id: u32) {
        if let Some(inner) = self.wrappers.get(&id).and_then(Weak::upgrade) {
            inner.mark_destroyed();
        }
        self.wrappers.remove(&id);
        self.widget_instances.remove(&id);
        self.compositor.clear_all();
    }

    fn mark_destroyed_many(&mut self, ids: &[u32]) {
        for id in ids {
            self.mark_destroyed(*id);
        }
    }

    fn register_widget_instance(
        &mut self,
        id: u32,
        instance: Arc<dyn widget_runtime::QtWidgetInstanceDyn>,
    ) -> Result<()> {
        if self.widget_instances.insert(id, instance).is_some() {
            return Err(invalid_arg(format!(
                "widget instance for node {id} is already registered"
            )));
        }

        Ok(())
    }

    fn widget_instance(&self, id: u32) -> Option<Arc<dyn widget_runtime::QtWidgetInstanceDyn>> {
        self.widget_instances.get(&id).cloned()
    }
}

static JS_CALLBACK: Lazy<Mutex<Option<Arc<EventCallback>>>> = Lazy::new(|| Mutex::new(None));
static CLEANUP_HOOK_INSTALLED: AtomicBool = AtomicBool::new(false);
static RUNTIME_STATE: Lazy<Mutex<RuntimeState>> = Lazy::new(|| Mutex::new(RuntimeState::new()));

pub(crate) fn ping() -> &'static str {
    "qt-solid-spike-native"
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

pub(crate) fn qt_error(message: impl Into<String>) -> Error {
    Error::new(Status::GenericFailure, message.into())
}

pub(crate) fn invalid_arg(message: impl Into<String>) -> Error {
    Error::new(Status::InvalidArg, message.into())
}

fn text_widget_type_id() -> WidgetTypeId {
    widget_registry()
        .bindings()
        .iter()
        .find(|binding| binding.kind_name == "text")
        .map(|binding| binding.widget_type_id)
        .expect("text widget binding")
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

fn type_is_non_negative(value_type: QtTypeInfo) -> bool {
    value_type.is_non_negative()
}

fn type_is_string(value_type: QtTypeInfo) -> bool {
    matches!(value_type.repr(), QtValueRepr::String)
}

fn type_is_bool(value_type: QtTypeInfo) -> bool {
    matches!(value_type.repr(), QtValueRepr::Bool)
}

fn type_is_i32_like(value_type: QtTypeInfo) -> bool {
    matches!(
        value_type.repr(),
        QtValueRepr::I32 { .. } | QtValueRepr::Enum(_)
    )
}

fn type_is_plain_i32(value_type: QtTypeInfo) -> bool {
    matches!(value_type.repr(), QtValueRepr::I32 { .. })
}

fn type_is_f64_like(value_type: QtTypeInfo) -> bool {
    matches!(value_type.repr(), QtValueRepr::F64 { .. })
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
        QtHostEvent::ListenerBatch {
            node_id, trace_id, ..
        } => (trace_id.unwrap_or(0) as u64, Some(*node_id), None),
        QtHostEvent::App { .. } | QtHostEvent::Debug { .. } | QtHostEvent::Inspect { .. } => {
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

fn widget_handle_for_inner(inner: &Arc<QtNodeInner>) -> widget_runtime::WidgetHandle {
    widget_runtime::WidgetHandle::new(NativeWidgetRuntimeHandle {
        inner: Arc::clone(inner),
    })
}

pub(crate) fn widget_instance_for_node_id(
    node_id: u32,
) -> Result<Arc<dyn widget_runtime::QtWidgetInstanceDyn>> {
    let generation = current_app_generation()?;
    let (inner, widget_type_id) = {
        let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(generation)?;
        if let Some(instance) = state.widget_instance(node_id) {
            return Ok(instance);
        }

        let inner = state
            .wrappers
            .get(&node_id)
            .and_then(Weak::upgrade)
            .ok_or_else(|| invalid_arg(format!("node {node_id} not found")))?;
        let NodeClass::Widget(widget_type_id) = inner.class else {
            return Err(invalid_arg(format!(
                "node {node_id} does not support widget instances"
            )));
        };

        (inner, widget_type_id)
    };

    let instance = create_widget_instance_for_inner(&inner, widget_type_id, Vec::new())?;
    let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(generation)?;
    if let Some(existing) = state.widget_instance(node_id) {
        return Ok(existing);
    }
    state.register_widget_instance(node_id, Arc::clone(&instance))?;
    Ok(instance)
}

pub(crate) fn qt_invoke_qpainter_hook(
    node_id: u32,
    _kind_tag: u8,
    hook_name: &str,
    painter: std::pin::Pin<&mut crate::qt::QPainter>,
) -> Result<()> {
    let instance = widget_instance_for_node_id(node_id)?;
    let mut host = QPainterOpaqueHost { raw: painter };
    if hook_name == "paint" {
        instance
            .paint(widget_runtime::PaintDevice::OpaqueHost(&mut host))
            .map_err(|error| qt_error(error.to_string()))
    } else {
        instance
            .invoke_host_override(hook_name, &mut host)
            .map_err(|error| qt_error(error.to_string()))
    }
}

pub(crate) fn ensure_live_node(node: &impl NodeHandle) -> Result<NodeClass> {
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

    Ok(node.inner().class)
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
    if state.tree.class(node_id).is_none() {
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
    widget_type_id: WidgetTypeId,
) -> Result<Arc<QtNodeInner>> {
    ensure_app_generation(generation)?;

    let (id, generation) = {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.allocate_node_id()?
    };

    qt::qt_create_widget(id, widget_registry().host_tag(widget_type_id))
        .map_err(|error| qt_error(error.what().to_owned()))?;

    let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(generation)?;
    state
        .tree
        .register(id, NodeClass::Widget(widget_type_id))
        .map_err(invalid_arg)?;
    let node = state.wrap_node(id)?;
    Ok(Arc::clone(node.inner()))
}

fn create_widget_instance_for_inner(
    inner: &Arc<QtNodeInner>,
    widget_type_id: WidgetTypeId,
    initial_props: Vec<QtInitialProp>,
) -> Result<Arc<dyn widget_runtime::QtWidgetInstanceDyn>> {
    let registry = widget_registry();
    let prop_decl = registry.prop_decl_for_widget_type_id(widget_type_id);
    let create_props = lower_widget_create_props(widget_type_id, initial_props)?;
    let create_instance = prop_decl
        .and_then(|decl| decl.create_instance)
        .or_else(|| {
            registry
                .native_decl_opt(registry.binding(widget_type_id).spec_key)
                .map(|decl| decl.create_instance)
        })
        .ok_or_else(|| {
            invalid_arg(format!(
                "missing widget constructor for {}",
                registry.binding(widget_type_id).kind_name
            ))
        })?;

    create_instance(widget_handle_for_inner(inner), &create_props)
        .map_err(|error| qt_error(error.to_string()))
}

pub(crate) fn attach_widget_instance(
    node: &impl NodeHandle,
    initial_props: Vec<QtInitialProp>,
) -> Result<Arc<dyn widget_runtime::QtWidgetInstanceDyn>> {
    let class = ensure_live_node(node)?;
    let NodeClass::Widget(widget_type_id) = class else {
        return Err(invalid_arg("only widget nodes can attach widget instances"));
    };
    let instance = create_widget_instance_for_inner(node.inner(), widget_type_id, initial_props)?;

    let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(node.inner().generation)?;
    state.register_widget_instance(node.inner().id, Arc::clone(&instance))?;
    Ok(instance)
}

pub(crate) fn create_widget_object(
    generation: u64,
    widget_type_id: WidgetTypeId,
) -> Result<Arc<QtNodeInner>> {
    let inner = create_widget_inner(generation, widget_type_id)?;
    let instance = create_widget_instance_for_inner(&inner, widget_type_id, Vec::new())?;

    let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
    state.ensure_generation(generation)?;
    state.register_widget_instance(inner.id, Arc::clone(&instance))?;

    Ok(inner)
}

pub(crate) fn create_text_node(generation: u64, value: String) -> Result<QtNode> {
    let inner = create_widget_object(generation, text_widget_type_id())?;
    let node = QtNode::from_inner(inner);
    crate::runtime::apply_string_prop_by_name(&node, "text", value)?;
    Ok(node)
}

pub(crate) fn create_widget_by_spec_key(generation: u64, spec_key: &str) -> Result<QtNode> {
    let binding = widget_registry()
        .binding_by_spec_key_str(spec_key)
        .ok_or_else(|| invalid_arg(format!("unknown widget spec key {spec_key}")))?;
    create_widget_object(generation, binding.widget_type_id).map(QtNode::from_inner)
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
            let class = state
                .tree
                .class(id)
                .ok_or_else(|| invalid_arg(format!("node {id} not found")))?;
            let parent_id = state.tree.get_parent(id);
            let children = state.tree.children(id).unwrap_or(&[]).to_vec();
            nodes.push((id, class, parent_id, children));
        }
        nodes
    };

    let mut nodes = Vec::new();
    for (id, class, parent_id, children) in nodes_to_snapshot {
        if matches!(class, NodeClass::Root) {
            nodes.push(QtDebugNodeSnapshot {
                id,
                kind: widget_registry().kind_name_for_node_class(class).to_owned(),
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
        let mut snapshot = snapshot_from_realized_state(id, class, parent_id, children, realized);
        snapshot.title = read_debug_string_prop(id, class, "title")?.or(snapshot.title);
        snapshot.text = read_debug_string_prop(id, class, "text")?.or(snapshot.text);
        snapshot.placeholder =
            read_debug_string_prop(id, class, "placeholder")?.or(snapshot.placeholder);
        snapshot.checked = read_debug_bool_prop(id, class, "checked")?.or(snapshot.checked);
        snapshot.value = read_debug_number_prop(id, class, "rangeValue")?.or(snapshot.value);
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
    class: NodeClass,
    parent_id: Option<u32>,
    children: Vec<u32>,
    realized: QtRealizedNodeState,
) -> QtDebugNodeSnapshot {
    QtDebugNodeSnapshot {
        id,
        kind: widget_registry().kind_name_for_node_class(class).to_owned(),
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

fn binding_prop_by_name(
    class: NodeClass,
    js_name: &str,
) -> Option<&'static crate::bootstrap::PropMeta> {
    widget_registry()
        .binding_for_node_class(class)
        .props
        .iter()
        .find(|prop| prop.js_name == js_name)
}

fn binding_prop_id_by_name(class: NodeClass, js_name: &str) -> Option<u16> {
    widget_registry().prop_id_for_class(class, js_name)
}

fn merged_prop_for_node(
    node: &impl NodeHandle,
    js_name: &str,
) -> Option<qt_solid_widget_core::schema::MergedProp> {
    let binding = node.inner().binding();
    let spec = widget_registry()
        .spec_bindings()
        .iter()
        .find(|spec| spec.spec_key == binding.spec_key)?;
    merged_props(spec, widget_registry().prop_decl(binding.spec_key))
        .into_iter()
        .find(|prop| prop.key == js_name)
}

fn prop_setter_slot_for_node(node: &impl NodeHandle, js_name: &str) -> Option<u16> {
    let prop = merged_prop_for_node(node, js_name)?;
    prop.write_slot()
}

fn prop_getter_slot_for_node(node: &impl NodeHandle, js_name: &str) -> Option<u16> {
    merged_prop_for_node(node, js_name)?.read_slot()
}

pub(crate) fn read_prop_exact(node: &impl NodeHandle, js_name: &str) -> Result<Option<QtValue>> {
    let Some(slot) = prop_getter_slot_for_node(node, js_name) else {
        return Ok(None);
    };

    let instance = widget_instance_for_node_id(node.inner().id)?;
    instance
        .read_prop(slot)
        .map(Some)
        .map_err(|error| qt_error(error.to_string()))
}

pub(crate) fn apply_prop_by_name(
    node: &impl NodeHandle,
    js_name: &str,
    value: QtValue,
) -> Result<Option<()>> {
    let Some(slot) = prop_setter_slot_for_node(node, js_name) else {
        return Ok(None);
    };

    let instance = widget_instance_for_node_id(node.inner().id)?;
    instance
        .apply_prop(slot, value)
        .map_err(|error| qt_error(error.to_string()))?;
    Ok(Some(()))
}

fn read_debug_string_prop(node_id: u32, class: NodeClass, js_name: &str) -> Result<Option<String>> {
    let Some(prop) = binding_prop_by_name(class, js_name) else {
        return Ok(None);
    };
    if prop.read_lowering.is_none() || !type_is_string(prop.value_type) {
        return Ok(None);
    }
    let Some(prop_id_value) = binding_prop_id_by_name(class, prop.js_name) else {
        return Ok(None);
    };
    qt::qt_read_string_prop(node_id, prop_id_value)
        .map(Some)
        .map_err(|error| qt_error(error.what().to_owned()))
}

fn read_debug_bool_prop(node_id: u32, class: NodeClass, js_name: &str) -> Result<Option<bool>> {
    let Some(prop) = binding_prop_by_name(class, js_name) else {
        return Ok(None);
    };
    if prop.read_lowering.is_none() || !type_is_bool(prop.value_type) {
        return Ok(None);
    }
    let Some(prop_id_value) = binding_prop_id_by_name(class, prop.js_name) else {
        return Ok(None);
    };
    qt::qt_read_bool_prop(node_id, prop_id_value)
        .map(Some)
        .map_err(|error| qt_error(error.what().to_owned()))
}

fn read_debug_number_prop(node_id: u32, class: NodeClass, js_name: &str) -> Result<Option<f64>> {
    let Some(prop) = binding_prop_by_name(class, js_name) else {
        return Ok(None);
    };
    if prop.read_lowering.is_none() {
        return Ok(None);
    }
    let Some(prop_id_value) = binding_prop_id_by_name(class, prop.js_name) else {
        return Ok(None);
    };
    if type_is_i32_like(prop.value_type) {
        qt::qt_read_i32_prop(node_id, prop_id_value)
            .map(|value| Some(value as f64))
            .map_err(|error| qt_error(error.what().to_owned()))
    } else if type_is_f64_like(prop.value_type) {
        qt::qt_read_f64_prop(node_id, prop_id_value)
            .map(Some)
            .map_err(|error| qt_error(error.what().to_owned()))
    } else {
        Ok(None)
    }
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

pub(crate) fn node_is_text_node(node: &impl NodeHandle) -> bool {
    !node.inner().is_destroyed() && node.inner().binding().kind_name == "text"
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
    let (previous_tree, next_tree) = {
        let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(parent.inner().generation)?;
        let mut next_tree = state.tree.clone();
        next_tree
            .insert_child(parent.inner().id, child.inner().id, anchor_id)
            .map_err(invalid_arg)?;
        (state.tree.clone(), next_tree)
    };

    {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(parent.inner().generation)?;
        state.tree = next_tree.clone();
    }

    if let Err(error) = qt::qt_insert_child(parent.inner().id, child.inner().id, anchor_id_or_zero)
    {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(parent.inner().generation)?;
        state.tree = previous_tree;
        return Err(qt_error(error.what().to_owned()));
    };
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

    let (previous_tree, next_tree) = {
        let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(parent.inner().generation)?;
        let mut next_tree = state.tree.clone();
        next_tree
            .remove_child(parent.inner().id, child.inner().id)
            .map_err(invalid_arg)?;
        (state.tree.clone(), next_tree)
    };

    {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(parent.inner().generation)?;
        state.tree = next_tree.clone();
    }

    if let Err(error) = qt::qt_remove_child(parent.inner().id, child.inner().id) {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(parent.inner().generation)?;
        state.tree = previous_tree;
        return Err(qt_error(error.what().to_owned()));
    };
    if let Some(window_id) = window_compositor::window_ancestor_id_for_node(
        parent.inner().generation,
        parent.inner().id,
    )? {
        window_compositor::mark_window_compositor_scene_subtree(window_id, parent.inner().id);
    }
    Ok(())
}

pub(crate) fn destroy_node(node: &impl NodeHandle) -> Result<()> {
    let class = ensure_live_node(node)?;
    if matches!(class, NodeClass::Root) {
        return Err(invalid_arg("cannot destroy the renderer root node"));
    }

    if node.inner().mark_destroyed_once() {
        return Ok(());
    }

    let (previous_tree, next_tree, removed_ids, parent_id, window_id) = {
        let state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(node.inner().generation)?;
        let mut next_tree = state.tree.clone();
        let parent_id = state.tree.get_parent(node.inner().id);
        let mut current = Some(node.inner().id);
        let mut window_id = None;
        while let Some(id) = current {
            let class = state
                .tree
                .class(id)
                .ok_or_else(|| invalid_arg(format!("node {id} not found")))?;
            if widget_registry().binding_for_node_class(class).kind_name == "window" {
                window_id = Some(id);
                break;
            }
            current = state.tree.get_parent(id);
        }
        let removed_ids = next_tree
            .remove_subtree(node.inner().id)
            .map_err(invalid_arg)?;
        (
            state.tree.clone(),
            next_tree,
            removed_ids,
            parent_id,
            window_id,
        )
    };

    {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(node.inner().generation)?;
        state.tree = next_tree;
    }

    if let Err(error) = qt::qt_destroy_widget(node.inner().id) {
        let mut state = RUNTIME_STATE.lock().expect("runtime state mutex poisoned");
        state.ensure_generation(node.inner().generation)?;
        state.tree = previous_tree;
        return Err(qt_error(error.what().to_owned()));
    }

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

fn trace_prop_apply_enter(
    stage: &'static str,
    node: &impl NodeHandle,
    prop_id_value: u16,
    js_name: &str,
) -> u64 {
    trace::record_current_static(
        "rust",
        stage,
        Some(node.inner().id),
        None,
        Some(prop_id_value),
        Some(js_name.to_owned()),
    )
}

fn trace_prop_apply_exit(
    stage: &'static str,
    node: &impl NodeHandle,
    prop_id_value: u16,
    js_name: &str,
    trace_id: u64,
) {
    trace::record_static(
        trace_id,
        "rust",
        stage,
        Some(node.inner().id),
        None,
        Some(prop_id_value),
        Some(js_name.to_owned()),
    );
}

pub(crate) fn qt_value_type_name(value: &QtValue) -> &'static str {
    match value {
        QtValue::Unit => "unit",
        QtValue::String(_) => "string",
        QtValue::Bool(_) => "boolean",
        QtValue::I32(_) => "i32",
        QtValue::F64(_) => "f64",
        QtValue::Enum(_) => "enum",
        QtValue::Color(_) => "QtColor",
        QtValue::Point(_) => "QtPoint",
        QtValue::Size(_) => "QtSize",
        QtValue::Rect(_) => "QtRect",
        QtValue::Affine(_) => "QtAffine",
    }
}

#[derive(Debug, PartialEq)]
enum ExactPropValue {
    String(String),
    Bool(bool),
    I32 { value: i32, non_negative: bool },
    F64 { value: f64, non_negative: bool },
    Enum { value: i32, max_tag: u8 },
}

fn lower_exact_prop_value(
    value_type: QtTypeInfo,
    value: QtValue,
) -> std::result::Result<ExactPropValue, QtValue> {
    match value {
        QtValue::String(value) if type_is_string(value_type) => Ok(ExactPropValue::String(value)),
        QtValue::Bool(value) if type_is_bool(value_type) => Ok(ExactPropValue::Bool(value)),
        QtValue::Enum(value) if matches!(value_type.repr(), QtValueRepr::Enum(_)) => {
            Ok(ExactPropValue::Enum {
                value,
                max_tag: value_type.enum_meta().expect("enum meta").values.len() as u8,
            })
        }
        QtValue::I32(value) if type_is_plain_i32(value_type) => Ok(ExactPropValue::I32 {
            value,
            non_negative: type_is_non_negative(value_type),
        }),
        QtValue::F64(value) if type_is_f64_like(value_type) => Ok(ExactPropValue::F64 {
            value,
            non_negative: type_is_non_negative(value_type),
        }),
        value => Err(value),
    }
}

pub(crate) fn apply_string_prop_exact(
    node: &impl NodeHandle,
    prop_id_value: u16,
    js_name: &'static str,
    value: String,
) -> Result<()> {
    ensure_live_node(node)?;

    let trace_id =
        trace_prop_apply_enter("rust.apply_string_prop.enter", node, prop_id_value, js_name);
    let result = qt::qt_apply_string_prop(node.inner().id, prop_id_value, trace_id, value.as_str())
        .map_err(|error| qt_error(error.what().to_owned()));
    trace_prop_apply_exit(
        "rust.apply_string_prop.exit",
        node,
        prop_id_value,
        js_name,
        trace_id,
    );
    result
}

pub(crate) fn apply_i32_prop_exact(
    node: &impl NodeHandle,
    prop_id_value: u16,
    js_name: &'static str,
    non_negative: bool,
    value: i32,
) -> Result<()> {
    ensure_live_node(node)?;

    if non_negative && value < 0 {
        return Err(invalid_arg(format!(
            "prop {} must be non-negative",
            js_name,
        )));
    }

    let trace_id =
        trace_prop_apply_enter("rust.apply_i32_prop.enter", node, prop_id_value, js_name);
    let result = qt::qt_apply_i32_prop(node.inner().id, prop_id_value, trace_id, value)
        .map_err(|error| qt_error(error.what().to_owned()));
    trace_prop_apply_exit(
        "rust.apply_i32_prop.exit",
        node,
        prop_id_value,
        js_name,
        trace_id,
    );
    result
}

pub(crate) fn apply_enum_prop_exact(
    node: &impl NodeHandle,
    prop_id_value: u16,
    js_name: &'static str,
    max_tag: u8,
    value: i32,
) -> Result<()> {
    ensure_live_node(node)?;

    let tag = u8::try_from(value).map_err(|_| {
        invalid_arg(format!(
            "enum prop {} received invalid tag {value}",
            js_name
        ))
    })?;
    if tag == 0 || tag > max_tag {
        return Err(invalid_arg(format!(
            "enum prop {} received invalid tag {value}",
            js_name,
        )));
    }

    let trace_id =
        trace_prop_apply_enter("rust.apply_i32_prop.enter", node, prop_id_value, js_name);
    let result = qt::qt_apply_i32_prop(node.inner().id, prop_id_value, trace_id, value)
        .map_err(|error| qt_error(error.what().to_owned()));
    trace_prop_apply_exit(
        "rust.apply_i32_prop.exit",
        node,
        prop_id_value,
        js_name,
        trace_id,
    );
    result
}

pub(crate) fn apply_f64_prop_exact(
    node: &impl NodeHandle,
    prop_id_value: u16,
    js_name: &'static str,
    non_negative: bool,
    value: f64,
) -> Result<()> {
    ensure_live_node(node)?;

    if non_negative && value < 0.0 {
        return Err(invalid_arg(format!(
            "prop {} must be non-negative",
            js_name,
        )));
    }

    let trace_id =
        trace_prop_apply_enter("rust.apply_f64_prop.enter", node, prop_id_value, js_name);
    let result = qt::qt_apply_f64_prop(node.inner().id, prop_id_value, trace_id, value)
        .map_err(|error| qt_error(error.what().to_owned()));
    trace_prop_apply_exit(
        "rust.apply_f64_prop.exit",
        node,
        prop_id_value,
        js_name,
        trace_id,
    );
    result
}

pub(crate) fn apply_bool_prop_exact(
    node: &impl NodeHandle,
    prop_id_value: u16,
    js_name: &'static str,
    value: bool,
) -> Result<()> {
    ensure_live_node(node)?;

    let trace_id =
        trace_prop_apply_enter("rust.apply_bool_prop.enter", node, prop_id_value, js_name);
    let result = qt::qt_apply_bool_prop(node.inner().id, prop_id_value, trace_id, value)
        .map_err(|error| qt_error(error.what().to_owned()));
    trace_prop_apply_exit(
        "rust.apply_bool_prop.exit",
        node,
        prop_id_value,
        js_name,
        trace_id,
    );
    result
}

pub(crate) fn apply_qt_prop_exact(
    node: &impl NodeHandle,
    prop_id_value: u16,
    value: QtValue,
) -> Result<()> {
    let prop = widget_registry()
        .prop_meta_for_class_id(node.inner().class, prop_id_value)
        .ok_or_else(|| invalid_arg(format!("unknown prop id {prop_id_value}")))?;

    if !prop.value_type.accepts_qt_value(&value) {
        return Err(invalid_arg(format!(
            "prop {} does not accept {} values",
            prop.js_name,
            qt_value_type_name(&value)
        )));
    }

    if let Some(slot) = prop_setter_slot_for_node(node, prop.js_name) {
        let instance = widget_instance_for_node_id(node.inner().id)?;
        return instance
            .apply_prop(slot, value)
            .map_err(|error| qt_error(error.to_string()));
    }

    match lower_exact_prop_value(prop.value_type, value) {
        Ok(ExactPropValue::String(value)) => {
            apply_string_prop_exact(node, prop_id_value, prop.js_name, value)
        }
        Ok(ExactPropValue::Bool(value)) => {
            apply_bool_prop_exact(node, prop_id_value, prop.js_name, value)
        }
        Ok(ExactPropValue::Enum { value, max_tag }) => {
            apply_enum_prop_exact(node, prop_id_value, prop.js_name, max_tag, value)
        }
        Ok(ExactPropValue::I32 {
            value,
            non_negative,
        }) => apply_i32_prop_exact(node, prop_id_value, prop.js_name, non_negative, value),
        Ok(ExactPropValue::F64 {
            value,
            non_negative,
        }) => apply_f64_prop_exact(node, prop_id_value, prop.js_name, non_negative, value),
        Err(value) => Err(invalid_arg(format!(
            "prop {} accepted {} but native runtime has no lowering for it yet",
            prop.js_name,
            qt_value_type_name(&value)
        ))),
    }
}

pub(crate) fn apply_qt_prop_by_name(
    node: &impl NodeHandle,
    js_name: &str,
    value: QtValue,
) -> Result<()> {
    if let Some(()) = apply_prop_by_name(node, js_name, value.clone())? {
        return Ok(());
    }

    let prop_id_value = binding_prop_id_by_name(node.inner().class, js_name)
        .ok_or_else(|| invalid_arg(format!("unknown prop {js_name}")))?;
    apply_qt_prop_exact(node, prop_id_value, value)
}

fn qt_value_to_method_value(value: QtValue) -> Result<qt::QtMethodValue> {
    match value {
        QtValue::Unit => Ok(qt::QtMethodValue {
            kind_tag: 0,
            string_value: String::new(),
            bool_value: false,
            i32_value: 0,
            f64_value: 0.0,
        }),
        QtValue::String(value) => Ok(qt::QtMethodValue {
            kind_tag: 1,
            string_value: value,
            bool_value: false,
            i32_value: 0,
            f64_value: 0.0,
        }),
        QtValue::Bool(value) => Ok(qt::QtMethodValue {
            kind_tag: 2,
            string_value: String::new(),
            bool_value: value,
            i32_value: 0,
            f64_value: 0.0,
        }),
        QtValue::I32(value) => Ok(qt::QtMethodValue {
            kind_tag: 3,
            string_value: String::new(),
            bool_value: false,
            i32_value: value,
            f64_value: 0.0,
        }),
        QtValue::F64(value) => Ok(qt::QtMethodValue {
            kind_tag: 4,
            string_value: String::new(),
            bool_value: false,
            i32_value: 0,
            f64_value: value,
        }),
        QtValue::Enum(value) => Ok(qt::QtMethodValue {
            kind_tag: 5,
            string_value: String::new(),
            bool_value: false,
            i32_value: value,
            f64_value: 0.0,
        }),
        QtValue::Color(_)
        | QtValue::Point(_)
        | QtValue::Size(_)
        | QtValue::Rect(_)
        | QtValue::Affine(_) => Err(invalid_arg(
            "native host-method bridge does not support render values yet",
        )),
    }
}

fn qt_value_to_method_value_core(
    value: QtValue,
) -> widget_runtime::WidgetResult<qt::QtMethodValue> {
    qt_value_to_method_value(value)
        .map_err(|error| widget_runtime::WidgetError::new(error.to_string()))
}

fn method_value_to_qt_value(value: qt::QtMethodValue) -> Result<QtValue> {
    match value.kind_tag {
        0 => Ok(QtValue::Unit),
        1 => Ok(QtValue::String(value.string_value)),
        2 => Ok(QtValue::Bool(value.bool_value)),
        3 => Ok(QtValue::I32(value.i32_value)),
        4 => Ok(QtValue::F64(value.f64_value)),
        5 => Ok(QtValue::Enum(value.i32_value)),
        kind_tag => Err(qt_error(format!(
            "host method returned unknown Qt value kind tag {kind_tag}"
        ))),
    }
}

pub(crate) fn call_host_method_exact(
    node: &impl NodeHandle,
    host_name: &str,
    args: Vec<QtValue>,
) -> Result<QtValue> {
    ensure_live_node(node)?;
    let binding = node.inner().binding();
    let method = binding
        .methods
        .host_methods
        .iter()
        .find(|method| method.host_name == host_name)
        .ok_or_else(|| {
            invalid_arg(format!(
                "widget {} has no host method named {}",
                binding.kind_name, host_name
            ))
        })?;
    let args = args
        .into_iter()
        .map(qt_value_to_method_value)
        .collect::<Result<Vec<_>>>()?;
    let value = qt::qt_call_host_slot(node.inner().id, method.slot, &args)
        .map_err(|error| qt_error(error.what().to_owned()))?;
    method_value_to_qt_value(value)
}

pub(crate) fn call_host_method_slot(
    node: &impl NodeHandle,
    slot: u16,
    args: Vec<QtValue>,
) -> Result<QtValue> {
    ensure_live_node(node)?;
    let binding = node.inner().binding();
    if !binding
        .methods
        .host_methods
        .iter()
        .any(|method| method.slot == slot)
    {
        return Err(invalid_arg(format!(
            "widget {} has no host method for slot {}",
            binding.kind_name, slot
        )));
    }
    let args = args
        .into_iter()
        .map(qt_value_to_method_value)
        .collect::<Result<Vec<_>>>()?;
    let value = qt::qt_call_host_slot(node.inner().id, slot, &args)
        .map_err(|error| qt_error(error.what().to_owned()))?;
    method_value_to_qt_value(value)
}

pub(crate) fn request_repaint_exact(node: &impl NodeHandle) -> Result<()> {
    ensure_live_node(node)?;
    if let Some(window_id) =
        window_compositor::window_ancestor_id_for_node(node.inner().generation, node.inner().id)?
    {
        window_compositor::qt_mark_window_compositor_pixels_dirty(window_id, node.inner().id);
    }
    qt::qt_request_repaint(node.inner().id).map_err(|error| qt_error(error.what().to_owned()))
}
pub(crate) fn capture_widget_exact(node: &impl NodeHandle) -> Result<WidgetCapture> {
    let class = ensure_live_node(node)?;
    let binding = widget_registry().binding_for_node_class(class);
    if binding.kind_name == "window" {
        return window_compositor::capture_window_widget_exact(node);
    }

    window_compositor::capture_painted_widget_exact_with_children(node, true)
}

pub(crate) fn apply_string_prop_by_id(
    node: &impl NodeHandle,
    prop_id_value: u16,
    value: String,
) -> Result<()> {
    let prop = widget_registry()
        .prop_meta_for_class_id(node.inner().class, prop_id_value)
        .ok_or_else(|| invalid_arg(format!("unknown prop id {prop_id_value}")))?;
    if type_is_string(prop.value_type) {
        apply_string_prop_exact(node, prop_id_value, prop.js_name, value)
    } else {
        Err(invalid_arg(format!(
            "prop {} does not accept string values",
            prop.js_name
        )))
    }
}

pub(crate) fn apply_string_prop_by_name(
    node: &impl NodeHandle,
    js_name: &str,
    value: String,
) -> Result<()> {
    if let Some(slot) = prop_setter_slot_for_node(node, js_name) {
        let instance = widget_instance_for_node_id(node.inner().id)?;
        instance
            .apply_prop(slot, QtValue::String(value))
            .map_err(|error| qt_error(error.to_string()))?;
        return Ok(());
    }

    let prop_id_value = binding_prop_id_by_name(node.inner().class, js_name)
        .ok_or_else(|| invalid_arg(format!("unknown prop {js_name}")))?;
    apply_string_prop_by_id(node, prop_id_value, value)
}

pub(crate) fn read_string_prop_by_name(node: &impl NodeHandle, js_name: &str) -> Result<String> {
    ensure_live_node(node)?;

    if let Some(value) = read_prop_exact(node, js_name)? {
        return match value {
            QtValue::String(value) => Ok(value),
            other => Err(invalid_arg(format!(
                "prop {js_name} returned {} instead of string",
                qt_value_type_name(&other)
            ))),
        };
    }

    let prop_id_value = binding_prop_id_by_name(node.inner().class, js_name)
        .ok_or_else(|| invalid_arg(format!("unknown prop {js_name}")))?;
    let prop = widget_registry()
        .prop_meta_for_class_id(node.inner().class, prop_id_value)
        .ok_or_else(|| invalid_arg(format!("unknown prop id {prop_id_value}")))?;
    if !type_is_string(prop.value_type) {
        return Err(invalid_arg(format!(
            "prop {} does not expose string reads",
            prop.js_name
        )));
    }

    qt::qt_read_string_prop(node.inner().id, prop_id_value)
        .map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn apply_bool_prop_by_id(
    node: &impl NodeHandle,
    prop_id_value: u16,
    value: bool,
) -> Result<()> {
    let prop = widget_registry()
        .prop_meta_for_class_id(node.inner().class, prop_id_value)
        .ok_or_else(|| invalid_arg(format!("unknown prop id {prop_id_value}")))?;
    if type_is_bool(prop.value_type) {
        apply_bool_prop_exact(node, prop_id_value, prop.js_name, value)
    } else {
        Err(invalid_arg(format!(
            "prop {} does not accept boolean values",
            prop.js_name
        )))
    }
}

pub(crate) fn apply_bool_prop_by_name(
    node: &impl NodeHandle,
    js_name: &str,
    value: bool,
) -> Result<()> {
    if let Some(()) = apply_prop_by_name(node, js_name, QtValue::Bool(value))? {
        return Ok(());
    }

    let prop_id_value = binding_prop_id_by_name(node.inner().class, js_name)
        .ok_or_else(|| invalid_arg(format!("unknown prop {js_name}")))?;
    apply_bool_prop_by_id(node, prop_id_value, value)
}

pub(crate) fn read_bool_prop_by_name(node: &impl NodeHandle, js_name: &str) -> Result<bool> {
    ensure_live_node(node)?;

    if let Some(value) = read_prop_exact(node, js_name)? {
        return match value {
            QtValue::Bool(value) => Ok(value),
            other => Err(invalid_arg(format!(
                "prop {js_name} returned {} instead of bool",
                qt_value_type_name(&other)
            ))),
        };
    }

    let prop_id_value = binding_prop_id_by_name(node.inner().class, js_name)
        .ok_or_else(|| invalid_arg(format!("unknown prop {js_name}")))?;
    let prop = widget_registry()
        .prop_meta_for_class_id(node.inner().class, prop_id_value)
        .ok_or_else(|| invalid_arg(format!("unknown prop id {prop_id_value}")))?;
    if !type_is_bool(prop.value_type) {
        return Err(invalid_arg(format!(
            "prop {} does not expose boolean reads",
            prop.js_name
        )));
    }

    qt::qt_read_bool_prop(node.inner().id, prop_id_value)
        .map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn apply_i32_prop_by_id(
    node: &impl NodeHandle,
    prop_id_value: u16,
    value: i32,
) -> Result<()> {
    let prop = widget_registry()
        .prop_meta_for_class_id(node.inner().class, prop_id_value)
        .ok_or_else(|| invalid_arg(format!("unknown prop id {prop_id_value}")))?;
    if let Some(domain) = prop.value_type.enum_meta() {
        apply_enum_prop_exact(
            node,
            prop_id_value,
            prop.js_name,
            domain.values.len() as u8,
            value,
        )
    } else if type_is_i32_like(prop.value_type) {
        apply_i32_prop_exact(
            node,
            prop_id_value,
            prop.js_name,
            type_is_non_negative(prop.value_type),
            value,
        )
    } else {
        Err(invalid_arg(format!(
            "prop {} does not accept integer values",
            prop.js_name
        )))
    }
}

pub(crate) fn apply_i32_prop_by_name(
    node: &impl NodeHandle,
    js_name: &str,
    value: i32,
) -> Result<()> {
    if let Some(()) = apply_prop_by_name(node, js_name, QtValue::I32(value))? {
        return Ok(());
    }

    let prop_id_value = binding_prop_id_by_name(node.inner().class, js_name)
        .ok_or_else(|| invalid_arg(format!("unknown prop {js_name}")))?;
    apply_i32_prop_by_id(node, prop_id_value, value)
}

pub(crate) fn read_i32_prop_by_name(node: &impl NodeHandle, js_name: &str) -> Result<i32> {
    ensure_live_node(node)?;

    if let Some(value) = read_prop_exact(node, js_name)? {
        return match value {
            QtValue::I32(value) | QtValue::Enum(value) => Ok(value),
            other => Err(invalid_arg(format!(
                "prop {js_name} returned {} instead of i32",
                qt_value_type_name(&other)
            ))),
        };
    }

    let prop_id_value = binding_prop_id_by_name(node.inner().class, js_name)
        .ok_or_else(|| invalid_arg(format!("unknown prop {js_name}")))?;
    let prop = widget_registry()
        .prop_meta_for_class_id(node.inner().class, prop_id_value)
        .ok_or_else(|| invalid_arg(format!("unknown prop id {prop_id_value}")))?;
    if !(type_is_i32_like(prop.value_type) || prop.value_type.enum_meta().is_some()) {
        return Err(invalid_arg(format!(
            "prop {} does not expose integer reads",
            prop.js_name
        )));
    }

    qt::qt_read_i32_prop(node.inner().id, prop_id_value)
        .map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn apply_f64_prop_by_id(
    node: &impl NodeHandle,
    prop_id_value: u16,
    value: f64,
) -> Result<()> {
    let prop = widget_registry()
        .prop_meta_for_class_id(node.inner().class, prop_id_value)
        .ok_or_else(|| invalid_arg(format!("unknown prop id {prop_id_value}")))?;
    if type_is_f64_like(prop.value_type) {
        apply_f64_prop_exact(
            node,
            prop_id_value,
            prop.js_name,
            type_is_non_negative(prop.value_type),
            value,
        )
    } else {
        Err(invalid_arg(format!(
            "prop {} does not accept number values",
            prop.js_name
        )))
    }
}

pub(crate) fn apply_f64_prop_by_name(
    node: &impl NodeHandle,
    js_name: &str,
    value: f64,
) -> Result<()> {
    if let Some(()) = apply_prop_by_name(node, js_name, QtValue::F64(value))? {
        return Ok(());
    }

    let prop_id_value = binding_prop_id_by_name(node.inner().class, js_name)
        .ok_or_else(|| invalid_arg(format!("unknown prop {js_name}")))?;
    apply_f64_prop_by_id(node, prop_id_value, value)
}

pub(crate) fn read_f64_prop_by_name(node: &impl NodeHandle, js_name: &str) -> Result<f64> {
    ensure_live_node(node)?;

    if let Some(value) = read_prop_exact(node, js_name)? {
        return match value {
            QtValue::F64(value) => Ok(value),
            other => Err(invalid_arg(format!(
                "prop {js_name} returned {} instead of f64",
                qt_value_type_name(&other)
            ))),
        };
    }

    let prop_id_value = binding_prop_id_by_name(node.inner().class, js_name)
        .ok_or_else(|| invalid_arg(format!("unknown prop {js_name}")))?;
    let prop = widget_registry()
        .prop_meta_for_class_id(node.inner().class, prop_id_value)
        .ok_or_else(|| invalid_arg(format!("unknown prop id {prop_id_value}")))?;
    if !type_is_f64_like(prop.value_type) {
        return Err(invalid_arg(format!(
            "prop {} does not expose number reads",
            prop.js_name
        )));
    }

    qt::qt_read_f64_prop(node.inner().id, prop_id_value)
        .map_err(|error| qt_error(error.what().to_owned()))
}

pub(crate) fn apply_enum_prop_by_id(
    node: &impl NodeHandle,
    prop_id_value: u16,
    value: &str,
) -> Result<()> {
    let prop = widget_registry()
        .prop_meta_for_class_id(node.inner().class, prop_id_value)
        .ok_or_else(|| invalid_arg(format!("unknown prop id {prop_id_value}")))?;
    let Some(domain) = prop.value_type.enum_meta() else {
        return Err(invalid_arg(format!(
            "prop {} does not accept enum values",
            prop.js_name
        )));
    };

    let tag = domain
        .values
        .iter()
        .position(|candidate| *candidate == value)
        .map(|index| (index + 1) as i32)
        .ok_or_else(|| {
            invalid_arg(format!(
                "invalid enum value {value} for prop {}",
                prop.js_name
            ))
        })?;

    apply_enum_prop_exact(
        node,
        prop_id_value,
        prop.js_name,
        domain.values.len() as u8,
        tag,
    )
}

pub(crate) fn apply_enum_prop_by_name(
    node: &impl NodeHandle,
    js_name: &str,
    value: &str,
) -> Result<()> {
    let prop_id_value = binding_prop_id_by_name(node.inner().class, js_name)
        .ok_or_else(|| invalid_arg(format!("unknown prop {js_name}")))?;
    apply_enum_prop_by_id(node, prop_id_value, value)
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
    request_repaint_exact(node)
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

fn emit_event_exports(
    node_id: u32,
    kind_tag: u8,
    event_index: u8,
    trace_id: u64,
    values: ListenerPayload,
) {
    let Some(widget_type_id) = widget_registry().widget_type_id_from_host_tag(kind_tag) else {
        return;
    };
    let Some(event) = widget_registry()
        .binding(widget_type_id)
        .events
        .get(usize::from(event_index))
    else {
        return;
    };

    if let Ok(instance) = widget_instance_for_node_id(node_id) {
        match event_values_to_qt_values(event, values.as_ref())
            .and_then(|qt_values| instance.invoke_host_event(event.rust_name, qt_values.as_slice()))
        {
            Ok(()) => {}
            Err(error) => {
                trace::record_static(
                    trace_id,
                    "rust",
                    "rust.host_event_error",
                    Some(node_id),
                    None,
                    None,
                    Some(error.to_string()),
                );
            }
        }
    }

    let mut export_ids = Vec::new();
    for export in event.exports {
        let Some(export_id_value) = widget_registry().export_id(export) else {
            continue;
        };
        trace::record_static(
            trace_id,
            "rust",
            "rust.emit_export",
            Some(node_id),
            Some(export_id_value),
            None,
            Some(export.to_string()),
        );
        export_ids.push(export_id_value);
    }

    let trace_id_value = (trace_id != 0).then_some(trace_id as i64);
    match export_ids.as_slice() {
        [] => {}
        [export_id] => emit_js_event(QtHostEvent::Listener {
            node_id,
            listener_id: *export_id,
            trace_id: trace_id_value,
            values: values.as_ref().to_vec(),
        }),
        _ => emit_js_event(QtHostEvent::ListenerBatch {
            node_id,
            listener_ids: export_ids,
            trace_id: trace_id_value,
            values: values.as_ref().to_vec(),
        }),
    }
}

pub(crate) fn emit_listener_event(
    node_id: u32,
    kind_tag: u8,
    event_index: u8,
    trace_id: u64,
    values: ListenerPayload,
) {
    emit_event_exports(node_id, kind_tag, event_index, trace_id, values);
}

fn event_value_to_qt_value(
    value_type: QtTypeInfo,
    value: &QtListenerValue,
) -> widget_runtime::WidgetResult<QtValue> {
    match value_type.repr() {
        QtValueRepr::String => Ok(QtValue::String(value.string_value.clone().ok_or_else(
            || widget_runtime::WidgetError::new("event payload missing string value"),
        )?)),
        QtValueRepr::Bool => Ok(QtValue::Bool(value.bool_value.ok_or_else(|| {
            widget_runtime::WidgetError::new("event payload missing bool value")
        })?)),
        QtValueRepr::I32 { .. } => {
            Ok(QtValue::I32(value.i32_value.ok_or_else(|| {
                widget_runtime::WidgetError::new("event payload missing i32 value")
            })?))
        }
        QtValueRepr::Enum(_) => Ok(QtValue::Enum(value.i32_value.ok_or_else(|| {
            widget_runtime::WidgetError::new("event payload missing enum value")
        })?)),
        QtValueRepr::F64 { .. } => {
            Ok(QtValue::F64(value.f64_value.ok_or_else(|| {
                widget_runtime::WidgetError::new("event payload missing f64 value")
            })?))
        }
        repr => Err(widget_runtime::WidgetError::new(format!(
            "unsupported event payload repr {:?}",
            repr
        ))),
    }
}

fn event_values_to_qt_values(
    event: &crate::bootstrap::EventMeta,
    values: &[QtListenerValue],
) -> widget_runtime::WidgetResult<Vec<QtValue>> {
    match event.payload_kind {
        crate::bootstrap::EventPayloadKind::Unit => {
            if !values.is_empty() {
                return Err(widget_runtime::WidgetError::new(
                    "unit event payload unexpectedly carried values",
                ));
            }
            Ok(Vec::new())
        }
        crate::bootstrap::EventPayloadKind::Scalar => {
            let payload_type = event.payload_type.ok_or_else(|| {
                widget_runtime::WidgetError::new("scalar event payload missing type info")
            })?;
            let value = values.first().ok_or_else(|| {
                widget_runtime::WidgetError::new("scalar event payload missing value")
            })?;
            Ok(vec![event_value_to_qt_value(payload_type, value)?])
        }
        crate::bootstrap::EventPayloadKind::Object => {
            if values.len() != event.payload_fields.len() {
                return Err(widget_runtime::WidgetError::new(format!(
                    "object event payload field count mismatch: expected {}, got {}",
                    event.payload_fields.len(),
                    values.len()
                )));
            }
            event
                .payload_fields
                .iter()
                .enumerate()
                .map(|(index, field)| event_value_to_qt_value(field.value_type, &values[index]))
                .collect()
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        collections::{HashMap, HashSet},
        sync::Arc,
    };

    use super::{
        ExactPropValue, ListenerPayload, QtListenerValue, RuntimeState, lower_exact_prop_value,
    };
    use crate::window_compositor::{
        WindowCaptureGrouping,
        pipeline::{
            WindowCaptureGroup, coalesce_scene_subtree_roots_in_tree, group_window_capture_parts,
            resize_reuse_cache_compatible, split_window_overlay_dirty_state,
            window_dirty_region_to_part_local_logical_rect,
        },
        prepare::{
            PixelRect, PremulPixel, build_prepared_window_compositor_frame,
            coalesce_pixel_rects_for_budget, collect_scene_node_dirty_regions,
            compose_window_capture_group, compose_window_capture_regions, read_capture_pixel,
            write_argb32_premultiplied_pixel,
        },
        state::{
            PartVisibleRect, WindowCaptureComposingPart, WindowCompositorCache,
            WindowCompositorDirtyFlags, WindowCompositorDirtyRegion, WindowCompositorLayerEntry,
            WindowCompositorLayerSourceKind, WindowCompositorPartUploadKind,
        },
    };
    use crate::{qt, window_compositor::prepare::vello_dirty_rects_to_local_pixel_rects};
    use qt_solid_runtime::tree::NodeTree;
    use qt_solid_widget_core::{
        decl::{FlexDirection, NodeClass, WidgetTypeId},
        runtime::{QtValue, WidgetCapture, WidgetCaptureFormat},
        schema::QtType,
        vello::VelloDirtyRect,
    };

    fn full_visible_rect(width: i32, height: i32) -> Vec<PartVisibleRect> {
        vec![PartVisibleRect {
            x: 0,
            y: 0,
            width,
            height,
        }]
    }

    fn capture_part(node_id: u32) -> WindowCaptureComposingPart {
        WindowCaptureComposingPart {
            node_id,
            x: 0,
            y: 0,
            width: 1,
            height: 1,
            visible_rects: full_visible_rect(1, 1),
            capture: WidgetCapture::new_zeroed(
                WidgetCaptureFormat::Argb32Premultiplied,
                1,
                1,
                4,
                1.0,
            )
            .expect("capture")
            .into(),
        }
    }

    fn layer_entry(part: WindowCaptureComposingPart) -> WindowCompositorLayerEntry {
        WindowCompositorLayerEntry::from_capture_part(
            part,
            WindowCompositorLayerSourceKind::CpuCapture,
        )
    }

    fn rgba_capture(red: u8, green: u8, blue: u8, alpha: u8) -> Arc<WidgetCapture> {
        let mut capture =
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Rgba8Premultiplied, 1, 1, 4, 1.0)
                .expect("capture");
        capture
            .bytes_mut()
            .copy_from_slice(&[red, green, blue, alpha]);
        Arc::new(capture)
    }

    fn argb_capture(pixel: PremulPixel) -> Arc<WidgetCapture> {
        let mut capture =
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 1, 1, 4, 1.0)
                .expect("capture");
        write_argb32_premultiplied_pixel(&mut capture, 0, 0, pixel);
        Arc::new(capture)
    }

    #[test]
    fn enum_props_lower_from_qt_enum_values() {
        assert_eq!(
            lower_exact_prop_value(<FlexDirection as QtType>::INFO, QtValue::Enum(1)),
            Ok(ExactPropValue::Enum {
                value: 1,
                max_tag: 2,
            })
        );
    }

    #[test]
    fn enum_props_do_not_fall_back_to_plain_i32_lowering() {
        assert_eq!(
            lower_exact_prop_value(<FlexDirection as QtType>::INFO, QtValue::I32(1)),
            Err(QtValue::I32(1))
        );
    }

    #[test]
    fn listener_payload_uses_shared_arc_slice_storage() {
        let payload: ListenerPayload = Arc::from(vec![QtListenerValue {
            path: "value".to_owned(),
            kind_tag: 1,
            string_value: Some("hello".to_owned()),
            bool_value: None,
            i32_value: None,
            f64_value: None,
        }]);
        let shared = Arc::clone(&payload);

        assert_eq!(Arc::strong_count(&payload), 2);
        assert_eq!(payload.as_ptr(), shared.as_ptr());
        assert_eq!(shared[0].path, "value");
        assert_eq!(shared[0].string_value.as_deref(), Some("hello"));
    }

    #[test]
    fn prepared_frame_part_bytes_borrow_capture_storage() {
        let capture = argb_capture(PremulPixel {
            red: 12,
            green: 34,
            blue: 56,
            alpha: 255,
        });
        let cache = WindowCompositorCache {
            generation: 1,
            width_px: 1,
            height_px: 1,
            stride: 4,
            scale_factor: 1.0,
            parts: vec![layer_entry(WindowCaptureComposingPart {
                node_id: 7,
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                visible_rects: full_visible_rect(1, 1),
                capture: Arc::clone(&capture),
            })],
        };

        let frame = build_prepared_window_compositor_frame(
            &cache,
            None,
            WindowCompositorDirtyFlags::from_bits(0),
            &HashSet::new(),
            &[],
            WindowCompositorPartUploadKind::Full,
        )
        .expect("frame");
        let bytes = frame
            .part(0)
            .expect("part")
            .capture
            .as_deref()
            .expect("cpu capture")
            .bytes();

        assert_eq!(bytes.as_ptr(), capture.bytes().as_ptr());
    }

    #[test]
    fn prepared_frame_cached_texture_part_omits_cpu_bytes() {
        let capture = argb_capture(PremulPixel {
            red: 1,
            green: 2,
            blue: 3,
            alpha: 255,
        });
        let cache = WindowCompositorCache {
            generation: 1,
            width_px: 1,
            height_px: 1,
            stride: 4,
            scale_factor: 1.0,
            parts: vec![WindowCompositorLayerEntry::from_capture_part(
                WindowCaptureComposingPart {
                    node_id: 7,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    visible_rects: full_visible_rect(1, 1),
                    capture,
                },
                WindowCompositorLayerSourceKind::CachedTexture,
            )],
        };

        let frame = build_prepared_window_compositor_frame(
            &cache,
            None,
            WindowCompositorDirtyFlags::from_bits(0),
            &HashSet::new(),
            &[],
            WindowCompositorPartUploadKind::Full,
        )
        .expect("frame");
        let part = frame.part(0).expect("part");

        assert_eq!(
            part.source_kind,
            WindowCompositorLayerSourceKind::CachedTexture
        );
        assert!(part.capture.is_none());
    }

    #[test]
    fn prepared_frame_pixel_dirty_uses_local_subrect_uploads() {
        let capture = Arc::new(
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 4, 4, 16, 1.0)
                .expect("capture"),
        );
        let part = WindowCaptureComposingPart {
            node_id: 7,
            x: 10,
            y: 20,
            width: 4,
            height: 4,
            visible_rects: full_visible_rect(4, 4),
            capture: Arc::clone(&capture),
        };
        let previous_cache = WindowCompositorCache {
            generation: 1,
            width_px: 64,
            height_px: 64,
            stride: 256,
            scale_factor: 1.0,
            parts: vec![layer_entry(part.clone())],
        };
        let current_cache = WindowCompositorCache {
            generation: 1,
            width_px: 64,
            height_px: 64,
            stride: 256,
            scale_factor: 1.0,
            parts: vec![layer_entry(part)],
        };
        let dirty_nodes = HashSet::from([7]);
        let dirty_regions = vec![WindowCompositorDirtyRegion {
            node_id: 7,
            x: 11,
            y: 21,
            width: 2,
            height: 1,
        }];

        let frame = build_prepared_window_compositor_frame(
            &current_cache,
            Some(&previous_cache),
            WindowCompositorDirtyFlags::PIXELS,
            &dirty_nodes,
            &dirty_regions,
            WindowCompositorPartUploadKind::None,
        )
        .expect("frame");
        let part = frame.part(0).expect("part");

        assert_eq!(part.upload_kind, WindowCompositorPartUploadKind::SubRects);
        assert_eq!(
            part.dirty_rects,
            vec![crate::qt::QtRect {
                x: 1,
                y: 1,
                width: 2,
                height: 1,
            }]
        );
        assert!(Arc::ptr_eq(
            part.capture.as_ref().expect("cpu capture"),
            &capture
        ));
    }

    #[test]
    fn prepared_frame_large_dirty_area_prefers_full_upload() {
        let capture = Arc::new(
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 4, 4, 16, 1.0)
                .expect("capture"),
        );
        let part = WindowCaptureComposingPart {
            node_id: 7,
            x: 10,
            y: 20,
            width: 4,
            height: 4,
            visible_rects: full_visible_rect(4, 4),
            capture: Arc::clone(&capture),
        };
        let previous_cache = WindowCompositorCache {
            generation: 1,
            width_px: 64,
            height_px: 64,
            stride: 256,
            scale_factor: 1.0,
            parts: vec![layer_entry(part.clone())],
        };
        let current_cache = WindowCompositorCache {
            generation: 1,
            width_px: 64,
            height_px: 64,
            stride: 256,
            scale_factor: 1.0,
            parts: vec![layer_entry(part)],
        };
        let dirty_nodes = HashSet::from([7]);
        let dirty_regions = vec![WindowCompositorDirtyRegion {
            node_id: 7,
            x: 10,
            y: 20,
            width: 3,
            height: 3,
        }];

        let frame = build_prepared_window_compositor_frame(
            &current_cache,
            Some(&previous_cache),
            WindowCompositorDirtyFlags::PIXELS,
            &dirty_nodes,
            &dirty_regions,
            WindowCompositorPartUploadKind::None,
        )
        .expect("frame");
        let part = frame.part(0).expect("part");

        assert_eq!(part.upload_kind, WindowCompositorPartUploadKind::Full);
        assert!(part.dirty_rects.is_empty());
    }

    #[test]
    fn prepared_frame_multiple_dirty_rects_prefers_full_upload() {
        let capture = Arc::new(
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 8, 8, 32, 1.0)
                .expect("capture"),
        );
        let part = WindowCaptureComposingPart {
            node_id: 7,
            x: 10,
            y: 20,
            width: 8,
            height: 8,
            visible_rects: full_visible_rect(8, 8),
            capture: Arc::clone(&capture),
        };
        let previous_cache = WindowCompositorCache {
            generation: 1,
            width_px: 64,
            height_px: 64,
            stride: 256,
            scale_factor: 1.0,
            parts: vec![layer_entry(part.clone())],
        };
        let current_cache = WindowCompositorCache {
            generation: 1,
            width_px: 64,
            height_px: 64,
            stride: 256,
            scale_factor: 1.0,
            parts: vec![layer_entry(part)],
        };
        let dirty_nodes = HashSet::from([7]);
        let dirty_regions = vec![
            WindowCompositorDirtyRegion {
                node_id: 7,
                x: 10,
                y: 20,
                width: 1,
                height: 1,
            },
            WindowCompositorDirtyRegion {
                node_id: 7,
                x: 16,
                y: 26,
                width: 1,
                height: 1,
            },
        ];

        let frame = build_prepared_window_compositor_frame(
            &current_cache,
            Some(&previous_cache),
            WindowCompositorDirtyFlags::PIXELS,
            &dirty_nodes,
            &dirty_regions,
            WindowCompositorPartUploadKind::None,
        )
        .expect("frame");
        let part = frame.part(0).expect("part");

        assert_eq!(part.upload_kind, WindowCompositorPartUploadKind::Full);
        assert!(part.dirty_rects.is_empty());
    }

    #[test]
    fn split_window_overlay_dirty_state_separates_base_from_overlay() {
        let cached_parts = vec![layer_entry(capture_part(7)), layer_entry(capture_part(8))];
        let dirty_nodes = HashSet::from([2, 7, 9]);
        let dirty_regions = vec![
            WindowCompositorDirtyRegion {
                node_id: 7,
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            },
            WindowCompositorDirtyRegion {
                node_id: 9,
                x: 5,
                y: 6,
                width: 7,
                height: 8,
            },
        ];

        let (
            overlay_dirty_nodes,
            overlay_dirty_regions,
            base_dirty_nodes,
            base_dirty_regions,
            overlay_frame_tick,
        ) = split_window_overlay_dirty_state(2, &cached_parts, &dirty_nodes, &dirty_regions);

        assert_eq!(overlay_dirty_nodes, HashSet::from([7]));
        assert_eq!(
            overlay_dirty_regions,
            vec![WindowCompositorDirtyRegion {
                node_id: 7,
                x: 1,
                y: 2,
                width: 3,
                height: 4,
            }]
        );
        assert_eq!(base_dirty_nodes, HashSet::from([9]));
        assert_eq!(
            base_dirty_regions,
            vec![WindowCompositorDirtyRegion {
                node_id: 9,
                x: 5,
                y: 6,
                width: 7,
                height: 8,
            }]
        );
        assert!(overlay_frame_tick);
    }

    #[test]
    fn window_compositor_dirty_flags_preserve_combined_bits() {
        let combined = WindowCompositorDirtyFlags::GEOMETRY | WindowCompositorDirtyFlags::PIXELS;

        assert_eq!(
            WindowCompositorDirtyFlags::from_bits(0),
            WindowCompositorDirtyFlags(0)
        );
        assert!(combined.contains(WindowCompositorDirtyFlags::GEOMETRY));
        assert!(combined.contains(WindowCompositorDirtyFlags::PIXELS));
        assert!(!combined.contains(WindowCompositorDirtyFlags::SCENE));
        assert_eq!(
            WindowCompositorDirtyFlags::from_bits(u8::MAX),
            WindowCompositorDirtyFlags(
                WindowCompositorDirtyFlags::GEOMETRY.0
                    | WindowCompositorDirtyFlags::SCENE.0
                    | WindowCompositorDirtyFlags::PIXELS.0
            )
        );
    }

    #[test]
    fn interactive_resize_cache_compat_ignores_window_dimensions() {
        let cache = WindowCompositorCache {
            generation: 7,
            width_px: 800,
            height_px: 600,
            stride: 3200,
            scale_factor: 2.0,
            parts: vec![layer_entry(capture_part(10))],
        };
        let layout = crate::qt::QtWidgetCaptureLayout {
            format_tag: 0,
            width_px: 1280,
            height_px: 720,
            stride: 5120,
            scale_factor: 2.0,
        };

        assert!(resize_reuse_cache_compatible(&cache, 7, &layout));
        assert!(!resize_reuse_cache_compatible(&cache, 8, &layout));
        assert!(!resize_reuse_cache_compatible(
            &cache,
            7,
            &crate::qt::QtWidgetCaptureLayout {
                scale_factor: 1.0,
                ..layout
            }
        ));
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
    fn composed_window_capture_blends_argb_and_rgba_parts() {
        let mut background =
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 1, 1, 4, 1.0)
                .expect("capture");
        write_argb32_premultiplied_pixel(
            &mut background,
            0,
            0,
            PremulPixel {
                red: 0,
                green: 0,
                blue: 200,
                alpha: 255,
            },
        );
        let background = Arc::new(background);

        let group = WindowCaptureGroup {
            parts: vec![
                WindowCaptureComposingPart {
                    node_id: 1,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    visible_rects: full_visible_rect(1, 1),
                    capture: background,
                },
                WindowCaptureComposingPart {
                    node_id: 2,
                    x: 0,
                    y: 0,
                    width: 1,
                    height: 1,
                    visible_rects: full_visible_rect(1, 1),
                    capture: rgba_capture(128, 0, 0, 128),
                },
            ],
        };

        let capture = compose_window_capture_group(1, 1, 4, 1.0, &group.parts)
            .expect("compose should succeed");
        assert_eq!(capture.format(), WidgetCaptureFormat::Argb32Premultiplied);
        assert_eq!(
            read_capture_pixel(&capture, 0, 0),
            PremulPixel {
                red: 128,
                green: 0,
                blue: 100,
                alpha: 255,
            }
        );
    }

    #[test]
    fn partial_compose_only_rebuilds_requested_region() {
        let mut base =
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 2, 1, 8, 1.0)
                .expect("capture");
        write_argb32_premultiplied_pixel(
            &mut base,
            0,
            0,
            PremulPixel {
                red: 10,
                green: 20,
                blue: 30,
                alpha: 255,
            },
        );
        write_argb32_premultiplied_pixel(
            &mut base,
            1,
            0,
            PremulPixel {
                red: 40,
                green: 50,
                blue: 60,
                alpha: 255,
            },
        );

        let parts = vec![
            WindowCaptureComposingPart {
                node_id: 1,
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                visible_rects: full_visible_rect(1, 1),
                capture: argb_capture(PremulPixel {
                    red: 200,
                    green: 0,
                    blue: 0,
                    alpha: 255,
                }),
            },
            WindowCaptureComposingPart {
                node_id: 2,
                x: 1,
                y: 0,
                width: 1,
                height: 1,
                visible_rects: full_visible_rect(1, 1),
                capture: argb_capture(PremulPixel {
                    red: 0,
                    green: 200,
                    blue: 0,
                    alpha: 255,
                }),
            },
        ];

        let capture = compose_window_capture_regions(
            &base,
            1.0,
            &parts,
            &[PixelRect {
                left: 0,
                top: 0,
                right: 1,
                bottom: 1,
            }],
        )
        .expect("partial compose should succeed");

        assert_eq!(
            read_capture_pixel(&capture, 0, 0),
            PremulPixel {
                red: 200,
                green: 0,
                blue: 0,
                alpha: 255,
            }
        );
        assert_eq!(
            read_capture_pixel(&capture, 1, 0),
            PremulPixel {
                red: 40,
                green: 50,
                blue: 60,
                alpha: 255,
            }
        );
    }

    #[test]
    fn scene_node_dirty_regions_union_old_and_new_bounds() {
        let target =
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 3, 1, 12, 1.0)
                .expect("capture");
        let dirty_nodes = HashSet::from([7_u32]);
        let old_parts = HashMap::from([(
            7_u32,
            WindowCaptureComposingPart {
                node_id: 7,
                x: 0,
                y: 0,
                width: 1,
                height: 1,
                visible_rects: full_visible_rect(1, 1),
                capture: argb_capture(PremulPixel {
                    red: 10,
                    green: 10,
                    blue: 10,
                    alpha: 255,
                }),
            },
        )]);
        let new_parts = HashMap::from([(
            7_u32,
            WindowCaptureComposingPart {
                node_id: 7,
                x: 2,
                y: 0,
                width: 1,
                height: 1,
                visible_rects: full_visible_rect(1, 1),
                capture: argb_capture(PremulPixel {
                    red: 20,
                    green: 20,
                    blue: 20,
                    alpha: 255,
                }),
            },
        )]);

        let regions = collect_scene_node_dirty_regions(
            target.width_px(),
            target.height_px(),
            1.0,
            &dirty_nodes,
            &old_parts,
            &new_parts,
        )
        .expect("collect should succeed");

        assert_eq!(regions.len(), 2);
        assert!(regions.contains(&PixelRect {
            left: 0,
            top: 0,
            right: 1,
            bottom: 1,
        }));
        assert!(regions.contains(&PixelRect {
            left: 2,
            top: 0,
            right: 3,
            bottom: 1,
        }));
    }

    #[test]
    fn compose_respects_part_visible_rects() {
        let mut capture =
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 2, 1, 8, 1.0)
                .expect("capture");
        write_argb32_premultiplied_pixel(
            &mut capture,
            0,
            0,
            PremulPixel {
                red: 220,
                green: 0,
                blue: 0,
                alpha: 255,
            },
        );
        write_argb32_premultiplied_pixel(
            &mut capture,
            1,
            0,
            PremulPixel {
                red: 0,
                green: 220,
                blue: 0,
                alpha: 255,
            },
        );

        let composed = compose_window_capture_group(
            2,
            1,
            8,
            1.0,
            &[WindowCaptureComposingPart {
                node_id: 9,
                x: 0,
                y: 0,
                width: 2,
                height: 1,
                visible_rects: vec![PartVisibleRect {
                    x: 1,
                    y: 0,
                    width: 1,
                    height: 1,
                }],
                capture: capture.into(),
            }],
        )
        .expect("compose should succeed");

        assert_eq!(
            read_capture_pixel(&composed, 0, 0),
            PremulPixel {
                red: 0,
                green: 0,
                blue: 0,
                alpha: 0,
            }
        );
        assert_eq!(
            read_capture_pixel(&composed, 1, 0),
            PremulPixel {
                red: 0,
                green: 220,
                blue: 0,
                alpha: 255,
            }
        );
    }

    #[test]
    fn scene_subtree_roots_coalesce_to_minimal_ancestors() {
        let mut tree = NodeTree::with_root(1);
        tree.register(2, NodeClass::Widget(WidgetTypeId::new(1)))
            .expect("register");
        tree.register(3, NodeClass::Widget(WidgetTypeId::new(1)))
            .expect("register");
        tree.register(4, NodeClass::Widget(WidgetTypeId::new(1)))
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

    #[test]
    fn vello_dirty_rects_scale_clip_and_merge() {
        let layout = qt::QtWidgetCaptureLayout {
            format_tag: 1,
            width_px: 100,
            height_px: 50,
            stride: 400,
            scale_factor: 2.0,
        };
        let regions = vello_dirty_rects_to_local_pixel_rects(
            &layout,
            &[
                VelloDirtyRect {
                    x: 10.0,
                    y: 5.0,
                    width: 8.0,
                    height: 4.0,
                },
                VelloDirtyRect {
                    x: 17.0,
                    y: 7.0,
                    width: 6.0,
                    height: 4.0,
                },
                VelloDirtyRect {
                    x: -2.0,
                    y: -1.0,
                    width: 3.0,
                    height: 3.0,
                },
            ],
        )
        .expect("dirty rect conversion should succeed");

        assert!(regions.contains(&PixelRect {
            left: 0,
            top: 0,
            right: 4,
            bottom: 6,
        }));
        assert!(regions.contains(&PixelRect {
            left: 18,
            top: 8,
            right: 48,
            bottom: 24,
        }));
    }

    #[test]
    fn vello_dirty_rects_merge_close_animation_regions_more_aggressively() {
        let layout = qt::QtWidgetCaptureLayout {
            format_tag: 1,
            width_px: 256,
            height_px: 256,
            stride: 1024,
            scale_factor: 1.0,
        };
        let regions = vello_dirty_rects_to_local_pixel_rects(
            &layout,
            &[
                VelloDirtyRect {
                    x: 48.0,
                    y: 52.0,
                    width: 118.0,
                    height: 116.0,
                },
                VelloDirtyRect {
                    x: 58.0,
                    y: 42.0,
                    width: 118.0,
                    height: 116.0,
                },
            ],
        )
        .expect("dirty rect conversion should succeed");

        assert_eq!(regions.len(), 1);
        assert_eq!(
            regions[0],
            PixelRect {
                left: 46,
                top: 40,
                right: 178,
                bottom: 170,
            }
        );
    }

    #[test]
    fn coalesce_pixel_rects_merges_close_regions_when_budget_allows() {
        let merged = coalesce_pixel_rects_for_budget(
            vec![
                PixelRect {
                    left: 10,
                    top: 10,
                    right: 20,
                    bottom: 20,
                },
                PixelRect {
                    left: 24,
                    top: 10,
                    right: 34,
                    bottom: 20,
                },
                PixelRect {
                    left: 38,
                    top: 10,
                    right: 48,
                    bottom: 20,
                },
            ],
            10_000,
            2,
            1.5,
            1.6,
            0.5,
        );

        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0],
            PixelRect {
                left: 10,
                top: 10,
                right: 48,
                bottom: 20,
            }
        );
    }

    #[test]
    fn coalesce_pixel_rects_preserves_far_apart_regions() {
        let merged = coalesce_pixel_rects_for_budget(
            vec![
                PixelRect {
                    left: 0,
                    top: 0,
                    right: 10,
                    bottom: 10,
                },
                PixelRect {
                    left: 80,
                    top: 80,
                    right: 90,
                    bottom: 90,
                },
                PixelRect {
                    left: 160,
                    top: 160,
                    right: 170,
                    bottom: 170,
                },
            ],
            40_000,
            2,
            1.2,
            1.2,
            0.2,
        );

        assert_eq!(merged.len(), 3);
    }

    #[test]
    fn window_dirty_region_maps_to_part_local_logical_rect() {
        let capture =
            WidgetCapture::new_zeroed(WidgetCaptureFormat::Argb32Premultiplied, 200, 120, 800, 2.0)
                .expect("capture");
        let part = WindowCaptureComposingPart {
            node_id: 7,
            x: 10,
            y: 8,
            width: 100,
            height: 60,
            visible_rects: Vec::new(),
            capture: Arc::new(capture),
        };

        assert_eq!(
            window_dirty_region_to_part_local_logical_rect(
                &part,
                WindowCompositorDirtyRegion {
                    node_id: 7,
                    x: 20,
                    y: 18,
                    width: 30,
                    height: 20,
                },
            ),
            Some(PartVisibleRect {
                x: 10,
                y: 10,
                width: 30,
                height: 20,
            })
        );
        assert_eq!(
            window_dirty_region_to_part_local_logical_rect(
                &part,
                WindowCompositorDirtyRegion {
                    node_id: 7,
                    x: -20,
                    y: -10,
                    width: 15,
                    height: 10,
                },
            ),
            None
        );
    }
}
